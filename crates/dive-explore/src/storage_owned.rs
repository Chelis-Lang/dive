//! Self-contained disk frontier for pure safety searches.
use super::storage::{Chunk, frontier_error};
use dive_core::owned_exchange::{decode_frontier, encode_frontier};
use dive_core::{Error, Handle, Heap, Store};
use dive_store::spill::{MAX_RECORD_BYTES, OrderedFrontier, SpillError};
use std::borrow::Cow;
use std::sync::Arc;

const ROWS: usize = 64;
const COMPRESSED: &[u8; 8] = b"DIVEFZ01";
const HEADER: usize = COMPRESSED.len() + size_of::<u32>();

fn pack<'a>(raw: &'a [u8], scratch: &'a mut Vec<u8>) -> Result<&'a [u8], SpillError> {
    let capacity = lz4_flex::block::get_maximum_output_size(raw.len()) + HEADER;
    if capacity > MAX_RECORD_BYTES || raw.len() <= HEADER {
        return Ok(raw);
    }
    scratch.clear();
    scratch.try_reserve(capacity).map_err(frontier_error)?;
    scratch.extend_from_slice(COMPRESSED);
    let len = u32::try_from(raw.len()).map_err(frontier_error)?;
    scratch.extend_from_slice(&len.to_le_bytes());
    scratch.resize(capacity, 0);
    let written =
        lz4_flex::block::compress_into(raw, &mut scratch[HEADER..]).map_err(frontier_error)?;
    if written + HEADER < raw.len() {
        Ok(&scratch[..written + HEADER])
    } else {
        Ok(raw)
    }
}

fn unpack(record: &[u8]) -> Result<Cow<'_, [u8]>, SpillError> {
    let Some(body) = record.strip_prefix(COMPRESSED) else {
        return Ok(Cow::Borrowed(record));
    };
    let (size, payload) = body
        .split_at_checked(size_of::<u32>())
        .ok_or_else(|| frontier_error("truncated compressed frontier header"))?;
    let len = u32::from_le_bytes(size.try_into().map_err(frontier_error)?) as usize;
    if len > MAX_RECORD_BYTES || len <= record.len() {
        return Err(frontier_error("invalid expanded frontier size"));
    }
    let mut raw = Vec::new();
    raw.try_reserve_exact(len).map_err(frontier_error)?;
    raw.resize(len, 0);
    let actual = lz4_flex::block::decompress_into(payload, &mut raw).map_err(frontier_error)?;
    if actual != len {
        return Err(frontier_error("expanded frontier length mismatch"));
    }
    Ok(Cow::Owned(raw))
}

pub(super) struct Owned {
    heap: Heap,
    width: usize,
    pending: Vec<Handle>,
    compression_scratch: Vec<u8>,
    pub(super) next: usize,
    pub(super) current: usize,
}

impl Owned {
    pub(super) fn new(store: &Arc<Store>, width: usize) -> Self {
        Self {
            heap: Heap::new(Arc::clone(store), true, false),
            width,
            pending: Vec::new(),
            compression_scratch: Vec::new(),
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
        self.next = self
            .next
            .checked_add(1)
            .ok_or_else(|| frontier_error("frontier row count overflow"))?;
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
            pack(&encoded, &mut self.compression_scratch)?,
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
        let roots =
            decode_frontier(heap, &unpack(&bytes)?, MAX_RECORD_BYTES).map_err(frontier_error)?;
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
        let handles = (end - start)
            .checked_mul(self.width)
            .ok_or_else(|| frontier_error("decoded frontier size overflow"))?;
        chunk
            .identity
            .try_reserve_exact(handles)
            .map_err(frontier_error)?;
        chunk
            .original
            .try_reserve_exact(handles)
            .map_err(frontier_error)?;
        chunk
            .ids
            .try_reserve_exact(end - start)
            .map_err(frontier_error)?;
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
                chunk.ids.push(
                    first
                        .checked_add(index as u64)
                        .ok_or_else(|| frontier_error("frontier discovery id overflow"))?,
                );
            }
        }
        Ok(Some(chunk))
    }

    pub(super) fn remapped(
        &self,
        remap: &mut dive_core::store::compact::Remap,
        poll: &(dyn Fn() -> bool + Sync),
    ) -> Result<Self, Error> {
        // Only the unfinished block contains coordinates from the old owner.
        // Leave every root intact if importing any one of them fails.
        let roots = self
            .pending
            .iter()
            .map(|h| remap.copy(*h, &|| !poll()))
            .collect::<Result<Vec<_>, _>>()?;
        Ok(Self {
            heap: Heap::new(Arc::clone(remap.store()), true, false),
            width: self.width,
            pending: roots,
            compression_scratch: Vec::new(),
            current: self.current,
            next: self.next,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::{COMPRESSED, HEADER, MAX_RECORD_BYTES, pack, unpack};

    #[test]
    #[allow(
        clippy::expect_used,
        reason = "successful bounded codec roundtrips are the test oracle"
    )]
    fn compressed_blocks_roundtrip_and_invalid_lengths_refuse() {
        let raw = b"DIVEOW02 contiguous repeated frontier roots".repeat(100);
        let mut scratch = Vec::new();
        let packed = pack(&raw, &mut scratch).expect("pack");
        assert!(packed.starts_with(COMPRESSED));
        assert!(packed.len() < raw.len());
        assert_eq!(unpack(packed).expect("unpack").as_ref(), raw);
        assert_eq!(
            unpack(b"DIVEOW02").expect("uncompressed").as_ref(),
            b"DIVEOW02"
        );
        assert!(unpack(COMPRESSED).is_err());
        for claimed in [0, raw.len() + 1, MAX_RECORD_BYTES + 1] {
            let mut broken = packed.to_vec();
            broken[COMPRESSED.len()..HEADER]
                .copy_from_slice(&u32::try_from(claimed).expect("test bound").to_le_bytes());
            assert!(unpack(&broken).is_err());
        }
        assert!(unpack(&packed[..=HEADER]).is_err());
    }
}
