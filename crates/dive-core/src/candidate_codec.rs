//! Owned, bounded candidate DAG records tied to one live Store.
//!
//! Transient payloads are copied before Arena rollback without interning them.
//! Only ordered consumption imports nodes into the
//! Store. This is transient working storage, not a checkpoint or trace proof.

use crate::handle::Tag;
use crate::node::{Kind, flags};
use crate::{Error, Handle, Heap, Store};
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

const MAGIC: &[u8; 8] = b"DIVECD01";
const HEADER: usize = 32;
const NODE_HEADER: usize = 42;
const REFERENCE: usize = 9;
const MAX_DEPTH: usize = 128;
/// Legacy report limit retained for schema stability; canonical records use zero.
pub const MAX_ORIGIN_INDEX_BYTES: usize = 1024 * 1024;
static NEXT: AtomicU64 = AtomicU64::new(1);

fn invalid(message: impl std::fmt::Display) -> Error {
    Error::internal(format!("native candidate payload: {message}"))
}

/// A private lifetime-bound transport for one source's ordered candidate rows.
#[derive(Debug)]
pub struct CandidateCodec {
    store: Arc<Store>,
    token: u64,
    source_arena: Arc<()>,
}

impl CandidateCodec {
    /// Retains the source's live Store and issues a nonreused lifetime token.
    ///
    /// # Errors
    /// The process lifetime token space is exhausted.
    pub fn new(heap: &Heap) -> Result<Self, Error> {
        let token = NEXT
            .try_update(Ordering::Relaxed, Ordering::Relaxed, |n| n.checked_add(1))
            .map_err(|_| invalid("lifetime tokens exhausted"))?;
        Ok(Self {
            store: Arc::clone(heap.store()),
            token,
            source_arena: Arc::clone(&heap.arena.cache_owner),
        })
    }

    /// Copies closed local roots into an owned child-first DAG without promotion.
    ///
    /// Existing persistent subtrees remain checked references to this live Store.
    /// Byte limits bound traversal/deduplication scratch. No new evaluation,
    /// normalization demand or persistent origin allocation is performed.
    ///
    /// # Errors
    /// A foreign/stale/lazy value, legacy history, nesting/byte limit or allocation.
    pub fn encode(
        &self,
        heap: &mut Heap,
        roots: &[Handle],
        limit: usize,
    ) -> Result<Vec<u8>, Error> {
        self.owner(heap)?;
        if !Arc::ptr_eq(&self.source_arena, &heap.arena.cache_owner) {
            return Err(invalid("foreign encoding Arena"));
        }
        if roots
            .len()
            .checked_mul(REFERENCE)
            .and_then(|n| n.checked_add(HEADER))
            .is_none_or(|n| n > limit)
        {
            return Err(Error::unsupported(
                "candidate root row exceeds its record bound",
            ));
        }
        let mut out = Write {
            bytes: Vec::new(),
            limit,
            nodes: HashMap::new(),
            scratch: 0,
        };
        out.put(MAGIC)?;
        out.word(self.token)?;
        out.word(0)?;
        out.word(roots.len() as u64)?;
        let mut refs = Vec::new();
        refs.try_reserve_exact(roots.len()).map_err(invalid)?;
        for &root in roots {
            refs.push(out.visit(heap, root, 0)?);
        }
        out.bytes[16..24].copy_from_slice(&(out.nodes.len() as u64).to_le_bytes());
        for reference in refs {
            out.reference(reference)?;
        }
        self.owner(heap)?;
        Ok(out.bytes)
    }

    /// Imports one unchanged record only when ordered commit consumes it.
    ///
    /// Shared children retain their aliases. Legacy constructor origins are refused.
    ///
    /// # Errors
    /// Invalid format, owner, shape, reference, allocation or construction origin.
    pub fn decode(
        &mut self,
        heap: &mut Heap,
        bytes: &[u8],
        limit: usize,
    ) -> Result<Vec<Handle>, Error> {
        self.owner(heap)?;
        if bytes.len() > limit {
            return Err(invalid("record exceeds byte bound"));
        }
        let mut input = Read { bytes, at: 0 };
        if input.take(8)? != MAGIC || input.word()? != self.token {
            return Err(invalid("version or lifetime owner mismatch"));
        }
        let count = input.count(NODE_HEADER)?;
        let roots = input.count(REFERENCE)?;
        let mut nodes = Vec::new();
        nodes.try_reserve_exact(count).map_err(invalid)?;
        for _ in 0..count {
            heap.work_poll()?;
            let kind = kind(input.take(1)?[0])?;
            let fl = input.take(1)?[0];
            let lo = input.word()?.cast_signed();
            let hash = input.word()?;
            let sum = input.word()?;
            let origin = input.word()?;
            let len = input.count(REFERENCE)?;
            let mut children = Vec::new();
            children.try_reserve_exact(len).map_err(invalid)?;
            for _ in 0..len {
                heap.work_poll()?;
                children.push(input.reference(&self.store, &nodes)?);
            }
            shape(kind, fl, lo, origin, &children)?;
            let handle = self
                .store
                .intern(&mut heap.cache, kind, fl, lo, hash, sum, &children);
            nodes.push(handle);
        }
        let mut values = Vec::new();
        values.try_reserve_exact(roots).map_err(invalid)?;
        for _ in 0..roots {
            heap.work_poll()?;
            values.push(input.reference(&self.store, &nodes)?);
        }
        if input.at != bytes.len() {
            return Err(invalid("trailing bytes"));
        }
        self.owner(heap)?;
        Ok(values)
    }

    /// Retained legacy report metric. Canonical candidates allocate no origins.
    #[must_use]
    pub const fn origin_index_bytes(&self) -> usize {
        0
    }

    fn owner(&self, heap: &Heap) -> Result<(), Error> {
        if !Arc::ptr_eq(&self.store, heap.store()) || heap.poisoned() || self.store.poisoned() {
            Err(invalid("foreign or poisoned owner"))
        } else {
            Ok(())
        }
    }
}

#[derive(Clone, Copy)]
struct Reference {
    local: bool,
    value: u64,
}

struct Write {
    bytes: Vec<u8>,
    limit: usize,
    nodes: HashMap<Handle, u64>,
    scratch: usize,
}

impl Write {
    fn put(&mut self, bytes: &[u8]) -> Result<(), Error> {
        let end = self
            .bytes
            .len()
            .checked_add(bytes.len())
            .filter(|&n| n <= self.limit)
            .ok_or_else(|| Error::unsupported("candidate payload exceeds its record bound"))?;
        if self.bytes.capacity() < end {
            let capacity = end
                .max(self.bytes.capacity().saturating_mul(2))
                .min(self.limit);
            self.bytes
                .try_reserve_exact(capacity - self.bytes.len())
                .map_err(invalid)?;
        }
        self.bytes.extend_from_slice(bytes);
        Ok(())
    }
    fn word(&mut self, value: u64) -> Result<(), Error> {
        self.put(&value.to_le_bytes())
    }
    fn reference(&mut self, reference: Reference) -> Result<(), Error> {
        self.put(&[u8::from(reference.local)])?;
        self.word(reference.value)
    }
    fn visit(&mut self, heap: &mut Heap, value: Handle, depth: usize) -> Result<Reference, Error> {
        heap.work_poll()?;
        if !value.is_transient() {
            checked(&heap.store, value.bits())?;
            return Ok(Reference {
                local: false,
                value: value.bits(),
            });
        }
        if depth >= MAX_DEPTH {
            return Err(Error::unsupported("candidate payload nesting exceeds 128"));
        }
        let origin = 0;
        if let Some(&value) = self.nodes.get(&value) {
            return Ok(Reference { local: true, value });
        }
        let view = heap
            .arena
            .lookup(value)
            .ok_or_else(|| invalid("stale Arena value"))?;
        if view.is_lazy() || view.kind.is_lazy() {
            return Err(invalid("unclosed lazy value"));
        }
        if view.flags & flags::HISTORY != 0 {
            return Err(invalid("unowned construction history"));
        }
        let scratch = view
            .data
            .len()
            .checked_mul(size_of::<Handle>() + size_of::<Reference>())
            .ok_or_else(|| invalid("traversal size overflow"))?;
        if view
            .data
            .len()
            .checked_mul(REFERENCE)
            .and_then(|n| n.checked_add(NODE_HEADER))
            .is_none_or(|n| n > self.limit.saturating_sub(self.bytes.len()))
            || self.nodes.len() >= self.limit / NODE_HEADER
            || scratch > self.limit.saturating_sub(self.scratch)
        {
            return Err(Error::unsupported("candidate DAG exceeds its record bound"));
        }
        self.scratch += scratch;
        let (kind, fl, lo, hash, sum) = (view.kind, view.flags, view.lo, view.hash, view.sum);
        let mut children = Vec::new();
        children
            .try_reserve_exact(view.data.len())
            .map_err(invalid)?;
        children.extend_from_slice(view.data);
        let mut refs = Vec::new();
        refs.try_reserve_exact(children.len()).map_err(invalid)?;
        for child in children {
            refs.push(self.visit(heap, child, depth + 1)?);
        }
        self.put(&[kind as u8, fl])?;
        self.word(lo.cast_unsigned())?;
        self.word(hash)?;
        self.word(sum)?;
        self.word(origin)?;
        self.word(refs.len() as u64)?;
        for reference in refs {
            self.reference(reference)?;
        }
        self.scratch -= scratch;
        let index = self.nodes.len() as u64;
        self.nodes.try_reserve(1).map_err(invalid)?;
        self.nodes.insert(value, index);
        Ok(Reference {
            local: true,
            value: index,
        })
    }
}

struct Read<'a> {
    bytes: &'a [u8],
    at: usize,
}
impl<'a> Read<'a> {
    fn take(&mut self, len: usize) -> Result<&'a [u8], Error> {
        let end = self
            .at
            .checked_add(len)
            .filter(|&n| n <= self.bytes.len())
            .ok_or_else(|| invalid("truncated record"))?;
        let bytes = &self.bytes[self.at..end];
        self.at = end;
        Ok(bytes)
    }
    fn word(&mut self) -> Result<u64, Error> {
        let mut bytes = [0; 8];
        bytes.copy_from_slice(self.take(8)?);
        Ok(u64::from_le_bytes(bytes))
    }
    fn count(&mut self, unit: usize) -> Result<usize, Error> {
        let count = usize::try_from(self.word()?).map_err(invalid)?;
        if count
            .checked_mul(unit)
            .is_none_or(|n| n > self.bytes.len().saturating_sub(self.at))
        {
            return Err(invalid("declared count exceeds record"));
        }
        Ok(count)
    }
    fn reference(&mut self, store: &Store, nodes: &[Handle]) -> Result<Handle, Error> {
        match self.take(1)?[0] {
            0 => checked(store, self.word()?),
            1 => usize::try_from(self.word()?)
                .ok()
                .and_then(|n| nodes.get(n))
                .copied()
                .ok_or_else(|| invalid("forward or missing local reference")),
            _ => Err(invalid("reference tag")),
        }
    }
}

fn checked(store: &Store, bits: u64) -> Result<Handle, Error> {
    let handle = Handle(bits);
    match handle.tag() {
        Tag::Int => Ok(handle),
        Tag::Empty if handle == Handle::EMPTY => Ok(handle),
        Tag::Bool if handle == Handle::TRUE || handle == Handle::FALSE => Ok(handle),
        Tag::Str | Tag::Model
            if u32::try_from(bits >> 3)
                .ok()
                .and_then(|id| store.strings().text(id))
                .is_some() =>
        {
            Ok(handle)
        }
        Tag::Persistent => {
            let view = store
                .retained_view(handle)
                .ok_or_else(|| invalid("unknown persistent coordinate"))?;
            if view.is_lazy() || view.kind.is_lazy() || view.flags & flags::HISTORY != 0 {
                Err(invalid("lazy or unowned persistent value"))
            } else {
                Ok(handle)
            }
        }
        _ => Err(invalid("invalid or transient external reference")),
    }
}

pub(crate) fn shape(
    kind: Kind,
    fl: u8,
    lo: i64,
    origin: u64,
    data: &[Handle],
) -> Result<(), Error> {
    let len = data.len();
    if data.iter().any(|h| h.is_empty())
        || fl & !flags::VIEW != 0
        || kind.is_lazy()
        || matches!(kind, Kind::Fcn | Kind::RecSet) && !len.is_multiple_of(2)
        || matches!(kind, Kind::Interval | Kind::Subset | Kind::SeqSet) && len != 1
        || matches!(kind, Kind::FcnSet | Kind::Union) && len != 2
        || matches!(kind, Kind::Nat | Kind::Int) && len != 0
        || kind == Kind::SetEnum && fl & flags::UNNORM == 0
        || fl & flags::TUPLE != 0 && (kind != Kind::IntFcn || lo != 1)
        || fl & flags::RECORD != 0 && kind != Kind::Fcn
        || fl & flags::UNNORM != 0 && kind != Kind::SetEnum
        || fl & flags::HISTORY != 0
        || origin != 0
        || kind == Kind::BigInt
            && (len == 0
                || !matches!(lo, -1 | 1)
                || data
                    .iter()
                    .any(|h| h.as_int().is_none_or(|n| u32::try_from(n).is_err()))
                || data.last().and_then(|h| h.as_int()) == Some(0))
    {
        return Err(invalid("node shape or representation flags"));
    }
    Ok(())
}

pub(crate) fn kind(byte: u8) -> Result<Kind, Error> {
    Ok(match byte {
        0 => Kind::Set,
        1 => Kind::IntFcn,
        2 => Kind::Fcn,
        3 => Kind::Interval,
        4 => Kind::Subset,
        5 => Kind::FcnSet,
        6 => Kind::RecSet,
        7 => Kind::SeqSet,
        8 => Kind::Nat,
        9 => Kind::Int,
        10 => Kind::Product,
        11 => Kind::Union,
        14 => Kind::BigInt,
        15 => Kind::SetEnum,
        _ => return Err(invalid("unknown or lazy node kind")),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, reason = "owned transient payload controls")]
mod tests {
    use super::*;

    #[test]
    fn transient_dags_wait_for_consumption_and_preserve_shared_children() {
        let store = Arc::new(Store::new(true));
        let mut heap = Heap::new(Arc::clone(&store), false, false);
        let mut codec = CandidateCodec::new(&heap).unwrap();
        let values: Vec<_> = (0..1024).map(|n| Handle::int(n).unwrap()).collect();
        let tuple = heap.tuple_literal(&values);
        let parent = heap.tuple_literal(&[tuple, tuple]);
        let before = store.stats();
        let bytes = codec.encode(&mut heap, &[parent, parent], 1 << 20).unwrap();
        assert_eq!(store.stats().nodes, before.nodes);
        assert_eq!(store.stats().handles, before.handles);
        assert!(bytes.len() < 12_000, "shared child encoded once");
        heap.reset();
        let roots = codec.decode(&mut heap, &bytes, 1 << 20).unwrap();
        assert_eq!(roots[0], roots[1]);
        assert_eq!(heap.view(roots[0]).flags & flags::TUPLE, flags::TUPLE);
        let child = heap.view(roots[0]).data[0];
        assert_eq!(child, heap.view(roots[0]).data[1]);
        assert_eq!(heap.view(child).data, values);
        assert_eq!(store.stats().nodes - before.nodes, 2);
    }

    #[test]
    fn canonical_candidates_keep_zero_origins_and_reject_legacy_history() {
        for semantics in [crate::Semantics::default(), crate::Semantics::tlc()] {
            let store = Arc::new(Store::with_semantics(true, semantics));
            let mut heap = Heap::new(Arc::clone(&store), false, false);
            let mut codec = CandidateCodec::new(&heap).unwrap();
            let one = Handle::int(1).unwrap();
            let value = heap.canonical_test_set(&[one, Handle::TRUE, one]).unwrap();
            let bytes = codec.encode(&mut heap, &[value, value], 4096).unwrap();
            assert_eq!(&bytes[58..66], &[0; 8], "legacy origin field is zero");
            heap.reset();
            for (at, byte) in [(58, 1), (33, flags::HISTORY)] {
                let mut forged = bytes.clone();
                forged[at] = byte;
                let before = store.stats().nodes;
                assert!(codec.decode(&mut heap, &forged, 4096).is_err());
                assert_eq!(
                    store.stats().nodes,
                    before,
                    "invalid origin cannot promote nodes"
                );
            }
            let roots = codec.decode(&mut heap, &bytes, 4096).unwrap();
            assert_eq!(roots[0], roots[1]);
            assert_eq!(heap.view(roots[0]).data, &[Handle::TRUE, one]);
            assert_eq!(codec.origin_index_bytes(), 0);
            assert_eq!(store.memory_usage().history_view_bytes, 0);
        }
    }

    #[test]
    fn local_identity_and_materialization_do_not_promote_or_fill_persistent_cache() {
        let store = Arc::new(Store::new(true));
        let mut heap = Heap::new(Arc::clone(&store), false, false);
        let interval = heap.interval(1, 128);
        let interval = heap.intern_const(interval).unwrap();
        let before = store.stats().nodes;
        let old = heap.set_speculative(true);
        let enumeration = heap.materialize_cached(interval).unwrap();
        let tuple = heap.tuple_literal(&[interval, enumeration]);
        let closed = heap.close_identity_local(tuple).unwrap();
        let expected = heap.tuple(&[enumeration, enumeration]);
        assert_eq!(heap.hash(closed), heap.hash(expected));
        assert!(heap.eq(closed, expected));
        assert!(enumeration.is_transient() && closed.is_transient());
        assert_eq!(heap.view(closed).flags, 0);
        assert_eq!(store.stats().nodes, before);
        heap.set_speculative(old);
        let retained = heap.intern(closed).unwrap();
        assert!(
            retained.is_persistent(),
            "explicit intern contract stays persistent"
        );
    }

    #[test]
    fn bounds_owner_corruption_lazy_and_stale_values_refuse() {
        let store = Arc::new(Store::new(true));
        let mut heap = Heap::new(Arc::clone(&store), false, false);
        let mut codec = CandidateCodec::new(&heap).unwrap();
        let tuple = heap.tuple_literal(&[Handle::TRUE; 1024]);
        assert!(codec.encode(&mut heap, &[tuple], 100).is_err());
        assert_eq!(store.stats().nodes, 0);
        let bytes = codec.encode(&mut heap, &[tuple], 1 << 20).unwrap();
        let mut other = Heap::new(Arc::clone(&store), false, false);
        let other_tuple = other.tuple_literal(&[Handle::TRUE]);
        assert!(codec.encode(&mut other, &[other_tuple], 1 << 20).is_err());
        let mut foreign = CandidateCodec::new(&heap).unwrap();
        assert!(foreign.decode(&mut heap, &bytes, 1 << 20).is_err());
        for end in [0, 7, 16, 31, 50, bytes.len() - 1] {
            assert!(codec.decode(&mut heap, &bytes[..end], 1 << 20).is_err());
        }
        let mut bad = bytes.clone();
        bad[32] = 255;
        assert!(codec.decode(&mut heap, &bad, 1 << 20).is_err());
        let mut bad = bytes.clone();
        let last = bad.len();
        bad[last - 9] = 1;
        bad[last - 8..].copy_from_slice(&u64::MAX.to_le_bytes());
        assert!(codec.decode(&mut heap, &bad, 1 << 20).is_err());
        heap.reset();
        assert!(codec.encode(&mut heap, &[tuple], 1 << 20).is_err());
    }
    #[test]
    fn speculative_step_memo_bypasses_promotion_even_when_general_memo_is_off() {
        use crate::memo::Memo;
        use crate::plan::{Code, MemoSite, Node};
        let store = Arc::new(Store::new(true));
        let opts = crate::Opts {
            memo: false,
            ..crate::Opts::default()
        };
        let mut machine = crate::Machine::new(Arc::clone(&store), 1, opts, Memo::new(&[1]));
        let next = machine.heap.tuple_literal(&[Handle::TRUE; 32]);
        machine.s1[0] = next;
        let code = Code {
            nodes: vec![
                Node::Primed(0),
                Node::Tuple(vec![0].into()),
                Node::Memo { site: 0, body: 1 },
            ],
            sites: vec![MemoSite {
                name: "step".into(),
                vars: vec![].into(),
                slots: vec![].into(),
                primed: vec![0].into(),
                step: true,
            }],
            ..Code::default()
        };
        let before = store.stats().nodes;
        machine.heap.set_speculative(true);
        let value = machine.eval(&code, 2).unwrap();
        assert!(value.is_transient());
        assert_eq!(
            store.stats().nodes,
            before,
            "step keys and results must not escape"
        );
        machine.heap.set_speculative(false);
        let value = machine.eval(&code, 2).unwrap();
        assert!(
            value.is_persistent(),
            "resident step memo still retains exact results"
        );
        assert!(store.stats().nodes > before);
    }
    #[test]
    fn local_identity_preserves_shared_dags_and_reuses_closed_roots() {
        for heap_mode in [false, true] {
            let store = Arc::new(Store::new(true));
            let mut heap = Heap::new(Arc::clone(&store), heap_mode, false);
            let mut root = Handle::FALSE;
            for _ in 0..20 {
                root = heap.tuple_literal(&[root, root]);
            }
            let before = heap.arena().len();
            let roots = heap.close_identity_row_local(&[root, root]).unwrap();
            assert_eq!(roots[0], roots[1]);
            let closed = roots[0];
            assert_eq!(
                heap.arena().len() - before,
                20,
                "one canonical header per original shared node"
            );
            let after = heap.arena().len();
            assert_eq!(heap.close_identity_local(closed).unwrap(), closed);
            assert_eq!(
                heap.arena().len(),
                after,
                "already-closed DAG adds no nodes or payload"
            );
            let mut walk = closed;
            for _ in 0..20 {
                let view = heap.view(walk);
                assert_eq!(view.flags, 0);
                assert_eq!(view.data[0], view.data[1]);
                walk = view.data[0];
            }
            let codec = CandidateCodec::new(&heap).unwrap();
            let bytes = codec.encode(&mut heap, &[closed], 4096).unwrap();
            assert!(
                bytes.len() < 1500,
                "twenty-node DAG must not become its expanded tree"
            );
            assert_eq!(store.stats().nodes, 0);
        }
    }

    #[test]
    fn local_closure_has_a_per_operation_visited_bound() {
        let store = Arc::new(Store::new(true));
        let mut heap = Heap::new(Arc::clone(&store), false, false);
        let children: Vec<_> = (0..=crate::heap::MAX_LOCAL_CLOSE_ENTRIES)
            .map(|_| heap.tuple_literal(&[Handle::TRUE]))
            .collect();
        let root = heap.tuple_literal(&children);
        let error = heap.close_identity_local(root).unwrap_err();
        assert!(error.message().contains("visited-node bound"));
        assert_eq!(store.stats().nodes, 0);
    }
}
