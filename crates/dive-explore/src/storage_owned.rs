//! Experimental self-contained disk frontier for pure safety searches.
use super::storage::{Chunk, frontier_error};
use dive_core::owned_exchange::{decode_frontier, encode_frontier};
use dive_core::{Error, Handle, Heap, Store};
use dive_store::spill::{MAX_RECORD_BYTES, OrderedFrontier, SpillError};
use std::sync::Arc;

const ROWS: usize = 64;

pub(super) struct Owned {
    heap: Heap,
    width: usize,
    pending: Vec<Handle>,
    pub(super) next: usize,
    pub(super) current: usize,
}

impl Owned {
    pub(super) fn new(store: &Arc<Store>, width: usize) -> Self {
        Self {
            heap: Heap::new(Arc::clone(store), true, false),
            width,
            pending: Vec::new(),
            next: 0,
            current: 0,
        }
    }

    pub(super) fn push(
        &mut self,
        queue: &mut OrderedFrontier,
        first: u64,
        identity: &[Handle],
        original: &[Handle],
    ) -> Result<(), SpillError> {
        if identity.len() != self.width || original.len() != self.width {
            return Err(frontier_error("wrong owned frontier row width"));
        }
        self.pending
            .try_reserve(2 * self.width)
            .map_err(frontier_error)?;
        self.pending.extend_from_slice(identity);
        self.pending.extend_from_slice(original);
        self.next += 1;
        if self.next.is_multiple_of(ROWS) {
            self.flush(queue, first)?;
        }
        Ok(())
    }

    pub(super) fn flush(
        &mut self,
        queue: &mut OrderedFrontier,
        first: u64,
    ) -> Result<(), SpillError> {
        let offset = queue
            .len()
            .checked_mul(ROWS)
            .ok_or_else(|| frontier_error("chunk offset overflow"))?;
        if offset >= self.next {
            return Ok(());
        }
        let encoded =
            encode_frontier(&self.heap, &self.pending, MAX_RECORD_BYTES).map_err(frontier_error)?;
        queue.push(
            first
                .checked_add(offset as u64)
                .ok_or_else(|| frontier_error("chunk id overflow"))?,
            &encoded,
        )?;
        self.pending.clear();
        Ok(())
    }

    fn block(
        heap: &mut Heap,
        queue: &OrderedFrontier,
        first: u64,
        width: usize,
        count: usize,
        i: usize,
    ) -> Result<Vec<Handle>, SpillError> {
        let (id, bytes) = queue.read(i)?;
        let offset = i * ROWS;
        if first.checked_add(offset as u64) != Some(id) {
            return Err(frontier_error("owned chunk discovery position"));
        }
        let roots = decode_frontier(heap, &bytes, MAX_RECORD_BYTES).map_err(frontier_error)?;
        if Some(roots.len())
            != (count - offset)
                .min(ROWS)
                .checked_mul(width)
                .and_then(|n| n.checked_mul(2))
        {
            return Err(frontier_error("invalid owned chunk width"));
        }
        Ok(roots)
    }

    pub(super) fn audit(
        &self,
        queue: &OrderedFrontier,
        first: u64,
        count: usize,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<bool, SpillError> {
        if queue.len() != count.div_ceil(ROWS) {
            return Err(frontier_error("missing owned chunk"));
        }
        let source = self.heap.store();
        let names = source.strings().texts_from(0);
        for i in 0..queue.len() {
            if poll() {
                return Ok(false);
            }
            let scratch = Store::with_semantics(true, source.semantics());
            scratch
                .strings()
                .preload(names.iter().map(AsRef::as_ref))
                .map_err(frontier_error)?;
            let mut heap = Heap::new(Arc::new(scratch), true, false);
            Self::block(&mut heap, queue, first, self.width, count, i)?;
        }
        Ok(true)
    }

    pub(super) fn chunk(
        &mut self,
        queue: &OrderedFrontier,
        first: u64,
        start: usize,
        end: usize,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<Option<Chunk>, SpillError> {
        if end > self.current || start > end {
            return Err(frontier_error("owned frontier range"));
        }
        let mut chunk = Chunk::default();
        for i in start / ROWS..end.div_ceil(ROWS) {
            if poll() {
                return Ok(None);
            }
            let roots = Self::block(&mut self.heap, queue, first, self.width, self.current, i)?;
            for index in start.max(i * ROWS)..end.min((i + 1) * ROWS) {
                let offset = (index % ROWS) * 2 * self.width;
                chunk
                    .identity
                    .extend_from_slice(&roots[offset..offset + self.width]);
                chunk
                    .original
                    .extend_from_slice(&roots[offset + self.width..offset + 2 * self.width]);
                chunk.ids.push(first + index as u64);
            }
        }
        Ok(Some(chunk))
    }

    pub(super) fn rebind(
        &mut self,
        remap: &mut dive_core::store::compact::Remap,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<(), Error> {
        // Only the unfinished block contains coordinates from the old owner.
        // Leave every root intact if importing any one of them fails.
        let roots = self
            .pending
            .iter()
            .map(|h| remap.copy(*h, &|| !poll()))
            .collect::<Result<Vec<_>, _>>()?;
        self.pending = roots;
        self.heap = Heap::new(Arc::clone(remap.store()), true, false);
        Ok(())
    }
}
