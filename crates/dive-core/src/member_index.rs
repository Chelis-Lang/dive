//! Bounded worker-local indexes, after normalized membership admission.

use std::sync::{Arc, Weak};

use crate::Error;
use crate::handle::{Handle, mix64};
use crate::heap::Heap;
use crate::node::Kind;
use crate::store::Store;

const RECORDS: usize = 64;
const MIN_MEMBERS: usize = 16;

#[derive(Debug)]
struct Record {
    set: Handle,
    revision: (u64, u64),
    slots: Vec<Handle>,
}

impl Default for Record {
    fn default() -> Self {
        Self {
            set: Handle::EMPTY,
            revision: (0, 0),
            slots: Vec::new(),
        }
    }
}

/// An index supplies only an extensional hit or miss, never history authority.
#[derive(Debug, Default)]
pub(crate) struct MemberIndex {
    pub(crate) cap: usize,
    records: Vec<Record>,
    store: Option<Weak<Store>>,
    arena: Option<Weak<()>>,
}

impl MemberIndex {
    pub(crate) fn clear(&mut self) {
        self.records = Vec::new();
        self.store = None;
        self.arena = None;
    }

    pub(crate) fn bytes(&self) -> usize {
        self.records.capacity() * std::mem::size_of::<Record>()
            + self
                .records
                .iter()
                .map(|r| r.slots.capacity() * 8)
                .sum::<usize>()
    }

    fn bind(&mut self, heap: &Heap) {
        if self
            .store
            .as_ref()
            .is_some_and(|s| std::ptr::eq(s.as_ptr(), Arc::as_ptr(&heap.store)))
            && self
                .arena
                .as_ref()
                .is_some_and(|a| std::ptr::eq(a.as_ptr(), Arc::as_ptr(&heap.arena.cache_owner)))
        {
            return;
        }
        self.clear();
        self.store = Some(Arc::downgrade(&heap.store));
        self.arena = Some(Arc::downgrade(&heap.arena.cache_owner));
    }

    // Only closed eager values whose existing hash agrees with canonical
    // comparison are admitted. This neither normalizes nor interns a query.
    fn eligible(heap: &Heap, value: Handle, depth: u8) -> Result<bool, Error> {
        heap.history_poll()?;
        if depth == 64 || value.is_empty() {
            return Ok(false);
        }
        if !value.is_compound() {
            return Ok(value
                .as_str_id()
                .or_else(|| value.as_model_id())
                .is_none_or(|id| heap.known_name(id)));
        }
        let view = heap.view(value);
        if view.is_lazy()
            || !matches!(
                view.kind,
                Kind::Set | Kind::IntFcn | Kind::Fcn | Kind::BigInt
            )
        {
            return Ok(false);
        }
        if view.kind == Kind::BigInt {
            return Ok(true);
        }
        for &child in view.data {
            if !Self::eligible(heap, child, depth + 1)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn bucket(hash: u64, slots: usize) -> usize {
        #[allow(
            clippy::cast_possible_truncation,
            reason = "masked to allocated table length"
        )]
        let bucket = mix64(hash) as usize & (slots - 1);
        bucket
    }

    /// `None` retains the old search. Admission and miss checks stay with
    /// the caller, including on every warm lookup.
    pub(crate) fn search(
        &mut self,
        heap: &Heap,
        set: Handle,
        query: Handle,
        values: &[Handle],
    ) -> Result<Option<bool>, Error> {
        if self.cap == 0
            || !query.is_compound()
            || values.len() < MIN_MEMBERS
            || heap.store.semantics().tlc_comparisons
        {
            return Ok(None);
        }
        if !Self::eligible(heap, query, 0)? {
            return Ok(None);
        }
        self.bind(heap);
        let metadata = RECORDS * std::mem::size_of::<Record>();
        if self.records.is_empty() {
            if metadata > self.cap || self.records.try_reserve_exact(RECORDS).is_err() {
                return Ok(None);
            }
            if self.records.capacity() * std::mem::size_of::<Record>() > self.cap {
                self.records = Vec::new();
                return Ok(None);
            }
            self.records.resize_with(RECORDS, Record::default);
        }
        let bucket = Self::bucket(set.bits(), RECORDS);
        let revision = if set.is_transient() {
            heap.arena.cache_revision()
        } else {
            (0, 0)
        };
        let record = &mut self.records[bucket];
        if record.set != set || record.revision != revision {
            *record = Record {
                set,
                revision,
                slots: Vec::new(),
            };
            return Ok(None); // First eligible lookup never builds an index.
        }
        if record.slots.is_empty() {
            let Some(size) = values
                .len()
                .checked_mul(2)
                .and_then(usize::checked_next_power_of_two)
            else {
                return Ok(None);
            };
            let Some(bytes) = size.checked_mul(8) else {
                return Ok(None);
            };
            if bytes > self.cap.saturating_sub(self.bytes()) {
                return Ok(None);
            }
            let mut slots = Vec::new();
            if slots.try_reserve_exact(size).is_err() {
                return Ok(None);
            }
            if slots.capacity() * 8 > self.cap.saturating_sub(self.bytes()) {
                return Ok(None);
            }
            slots.resize(size, Handle::EMPTY);
            for &value in values {
                if !Self::eligible(heap, value, 0)? {
                    return Ok(None);
                }
                let mut pos = Self::bucket(heap.hash(value), size);
                while !slots[pos].is_empty() {
                    heap.history_poll()?;
                    pos = (pos + 1) & (size - 1);
                }
                slots[pos] = value;
            }
            self.records[bucket].slots = slots;
        }
        let slots = &self.records[bucket].slots;
        let mut pos = Self::bucket(heap.hash(query), slots.len());
        while !slots[pos].is_empty() {
            heap.history_poll()?;
            // Equal hashes and equal persistent coordinates alone prove
            // nothing about independently represented construction values.
            if heap.cmp(slots[pos], query).is_eq() {
                return Ok(Some(true));
            }
            pos = (pos + 1) & (slots.len() - 1);
        }
        Ok(Some(false))
    }
}

#[cfg(test)]
#[allow(
    clippy::unwrap_used,
    reason = "bounded exact fixture construction and expected successful calls"
)]
mod tests {
    use super::*;
    use crate::memo::Memo;
    use crate::{Machine, Opts, Semantics};

    fn machine(tlc: bool, hashcons: bool) -> Machine {
        let semantics = if tlc {
            Semantics::tlc()
        } else {
            Semantics::default()
        };
        Machine::new(
            Arc::new(Store::with_semantics(hashcons, semantics)),
            0,
            Opts::default(),
            Memo::default(),
        )
    }

    fn tuple_set(m: &mut Machine) -> Handle {
        let values: Vec<_> = (0..32)
            .map(|i| {
                let x = Handle::int(i).unwrap();
                m.heap.tuple_literal(&[x, x])
            })
            .collect();
        let set = m.heap.local_literal(&values).unwrap();
        m.heap.norm(set).unwrap()
    }

    fn indexed(m: &Machine) -> usize {
        m.memo
            .member_index
            .records
            .iter()
            .filter(|r| !r.slots.is_empty())
            .count()
    }

    #[test]
    fn second_normalized_lookup_builds_and_preserves_all_hits_and_errors() {
        for tlc in [false, true] {
            for hashcons in [false, true] {
                let mut m = machine(tlc, hashcons);
                let set = tuple_set(&mut m);
                let query = m.heap.tuple_literal(&[Handle::int(4).unwrap(); 2]);
                assert_eq!(m.member(query, set), Ok(true));
                assert_eq!(indexed(&m), 0);
                assert_eq!(m.member(query, set), Ok(true));
                assert_eq!(indexed(&m), usize::from(!tlc));
                for i in -2..35 {
                    let query = m.heap.tuple_literal(&[Handle::int(i).unwrap(); 2]);
                    let old = m.heap.literal_member(query, set).unwrap();
                    assert_eq!(m.member(query, set), old);
                }
                for query in [Handle::TRUE, Handle::int(4).unwrap()] {
                    // Failing checked comparisons change their receiver's
                    // phase. Differential receivers therefore stay independent.
                    let candidate = tuple_set(&mut m);
                    let reference = tuple_set(&mut m);
                    let hit = m.heap.view(candidate).data[0];
                    for _ in 0..2 {
                        assert_eq!(m.member(hit, candidate), Ok(true));
                    }
                    let old = m.heap.literal_member(query, reference).unwrap();
                    assert_eq!(m.member(query, candidate), old);
                }
                assert!(!m.heap.poisoned());
            }
        }
    }

    #[test]
    fn raw_empty_small_and_disabled_paths_never_allocate_indexes() {
        for disabled in [false, true] {
            let mut m = machine(false, true);
            if disabled {
                m.memo.reserve_member_index(false);
            }
            let values: Vec<_> = (0..32).map(|i| Handle::int(i).unwrap()).collect();
            let raw = m.heap.local_literal(&values).unwrap();
            for _ in 0..4 {
                assert_eq!(m.member(values[0], raw), Ok(true));
            }
            let empty = m.heap.local_literal(&[]).unwrap();
            for _ in 0..4 {
                assert_eq!(m.member(Handle::EMPTY, empty), Ok(false));
            }
            let small = m.heap.local_literal(&values[..15]).unwrap();
            let small = m.heap.norm(small).unwrap();
            for _ in 0..4 {
                assert_eq!(m.member(values[0], small), Ok(true));
            }
            if disabled {
                let norm = m.heap.norm(raw).unwrap();
                for _ in 0..4 {
                    assert_eq!(m.member(values[0], norm), Ok(true));
                }
            }
            assert_eq!(m.memo.member_index.bytes(), 0);
            assert!(!m.heap.poisoned());
        }
        assert!(!Opts::parse("-member-index").unwrap().member_index);
        assert_eq!(
            Opts::parse("-member-index").unwrap().describe(),
            "-member-index"
        );
    }

    #[test]
    fn equal_hashes_require_original_structural_comparison() {
        let mut m = machine(false, true);
        let mut values: Vec<_> = (0..32)
            .map(|i| {
                m.heap
                    .arena
                    .alloc(Kind::IntFcn, 0, 1, 7, 0, &[Handle::int(i).unwrap()])
            })
            .collect();
        let set = m.heap.set_from(&mut values);
        for i in -1..33 {
            let query = m
                .heap
                .arena
                .alloc(Kind::IntFcn, 0, 1, 7, 0, &[Handle::int(i).unwrap()]);
            for _ in 0..2 {
                assert_eq!(m.member(query, set), Ok((0..32).contains(&i)));
            }
        }
        assert_eq!(indexed(&m), 1);
        assert!(!m.heap.poisoned());
    }

    #[test]
    fn local_indexes_do_not_survive_rollback_reset_or_replaced_arena() {
        let mut m = machine(false, true);
        let set = tuple_set(&mut m);
        let query = m.heap.view(set).data[0];
        let mark = m.heap.mark();
        for _ in 0..2 {
            assert_eq!(m.member(query, set), Ok(true));
        }
        assert_eq!(indexed(&m), 1);
        let _temporary = m.heap.tuple(&[Handle::TRUE]);
        m.heap.rollback(mark);
        assert_eq!(m.member(query, set), Ok(true));
        assert_eq!(indexed(&m), 0);
        assert_eq!(m.member(query, set), Ok(true));
        assert_eq!(indexed(&m), 1);
        m.heap.reset();
        let replacement = tuple_set(&mut m);
        let query = m.heap.view(replacement).data[0];
        assert_eq!(m.member(query, replacement), Ok(true));
        assert!(!m.heap.poisoned());
        m.heap = Heap::new(Arc::clone(m.heap.store()), false, false);
        let replacement = tuple_set(&mut m);
        let query = m.heap.view(replacement).data[0];
        assert_eq!(m.member(query, replacement), Ok(true));
        assert_eq!(indexed(&m), 0);
        assert_eq!(m.member(query, replacement), Ok(true));
        assert_eq!(indexed(&m), 1);
    }

    #[test]
    fn persistent_indexes_rebind_store_and_obey_cap_shrink_and_eviction() {
        for cap in [0, 2048, 1 << 16, 1 << 20] {
            let mut m = machine(false, true);
            m.memo.set_cap(cap);
            let set = tuple_set(&mut m);
            let set = m.heap.intern_exact(set).unwrap();
            m.heap.reset();
            for i in 0..4 {
                let query = m.heap.tuple_literal(&[Handle::int(i).unwrap(); 2]);
                assert_eq!(m.member(query, set), Ok(true));
            }
            assert!(m.memo.bytes() <= cap);
            assert!(m.memo.member_index.bytes() <= m.memo.member_index.cap);
            m.memo.set_cap(0);
            assert_eq!(m.memo.member_index.bytes(), 0);
            let mut moved = machine(false, true);
            moved.memo = std::mem::take(&mut m.memo);
            moved.memo.set_cap(1 << 20);
            let set = tuple_set(&mut moved);
            let set = moved.heap.intern_exact(set).unwrap();
            let query = moved.heap.tuple_literal(&[Handle::int(3).unwrap(); 2]);
            assert_eq!(moved.member(query, set), Ok(true));
            assert!(!moved.heap.poisoned());
        }
    }

    #[test]
    fn independent_history_values_keep_extensional_collision_verification() {
        let mut m = machine(false, true);
        let mut values = Vec::new();
        for i in 0..32 {
            let inner = m.heap.local_literal(&[Handle::int(i).unwrap()]).unwrap();
            let inner = m.heap.norm(inner).unwrap();
            let tuple = m.heap.tuple_literal(&[inner]);
            values.push(m.heap.intern_exact(tuple).unwrap());
        }
        let set = m.heap.set_from(&mut values);
        let set = m.heap.intern_exact(set).unwrap();
        let inner = m.heap.local_literal(&[Handle::int(5).unwrap()]).unwrap();
        let inner = m.heap.norm(inner).unwrap();
        let query = m.heap.tuple_literal(&[inner]);
        let query = m.heap.intern_exact(query).unwrap();
        let actual = values
            .iter()
            .find(|&&h| m.heap.cmp(h, query).is_eq())
            .copied()
            .unwrap();
        assert_ne!(actual, query);
        assert!(m.heap.cmp(actual, query).is_eq());
        for _ in 0..3 {
            assert_eq!(m.member(query, set), Ok(true));
        }
        assert_eq!(indexed(&m), 1);
        assert!(!m.heap.poisoned());
    }

    #[test]
    fn tuple_and_generic_integer_function_compare_equal_without_handle_equality() {
        let mut m = machine(false, true);
        let mut values: Vec<_> = (0..32)
            .map(|i| {
                let tuple = m.heap.tuple(&[Handle::int(i).unwrap()]);
                m.heap.intern(tuple).unwrap()
            })
            .collect();
        let actual = values[5];
        let set = m.heap.set_from(&mut values);
        let set = m.heap.intern(set).unwrap();
        // The ordinary builder canonicalizes integer domains to IntFcn.
        // Build the equally valid generic representation directly instead.
        let view = m.heap.view(actual);
        let query = m.heap.store.intern(
            &mut crate::store::InternCache::default(),
            Kind::Fcn,
            0,
            0,
            view.hash,
            view.sum,
            &[Handle::int(1).unwrap(), Handle::int(5).unwrap()],
        );
        assert!(m.heap.cmp(actual, query).is_eq());
        assert!(!m.heap.eq(actual, query));
        for _ in 0..3 {
            assert_eq!(m.member(query, set), Ok(true));
        }
        assert_eq!(indexed(&m), 1);
        assert!(!m.heap.poisoned());
    }

    #[test]
    fn warm_cache_rebinds_to_a_different_store_without_using_old_handles() {
        let mut m = machine(false, true);
        let set = tuple_set(&mut m);
        let set = m.heap.intern_exact(set).unwrap();
        let query = m.heap.view(set).data[0];
        for _ in 0..2 {
            assert_eq!(m.member(query, set), Ok(true));
        }
        assert_eq!(indexed(&m), 1);
        let old = Arc::downgrade(m.heap.store());
        m.heap = Heap::new(Arc::new(Store::default()), false, false);
        let mut values: Vec<_> = (100..132)
            .map(|i| m.heap.tuple(&[Handle::int(i).unwrap()]))
            .collect();
        let new = m.heap.set_from(&mut values);
        let new = m.heap.intern_exact(new).unwrap();
        let miss = m.heap.tuple(&[Handle::int(0).unwrap()]);
        assert_eq!(m.member(miss, new), Ok(false));
        assert_eq!(indexed(&m), 0);
        assert!(old.upgrade().is_none());
        let hit = m.heap.tuple(&[Handle::int(100).unwrap()]);
        assert_eq!(m.member(hit, new), Ok(true));
        assert_eq!(indexed(&m), 1);
        assert!(!m.heap.poisoned());
    }

    #[test]
    fn body_tables_share_the_reserved_total_and_replacement_only_drops_indexes() {
        let mut m = machine(false, true);
        m.memo = Memo::new(&[1]);
        m.memo.set_cap(1 << 20);
        m.memo.reserve_member_index(true);
        for i in 0..10000 {
            let key = Handle::int(i).unwrap();
            m.memo.put(0, &[key], Handle::TRUE);
        }
        let mut sets = Vec::new();
        for i in 0..256 {
            let mut values: Vec<_> = (0..32)
                .map(|v| m.heap.tuple(&[Handle::int(i * 32 + v).unwrap()]))
                .collect();
            let set = m.heap.set_from(&mut values);
            sets.push((set, values[0]));
            for _ in 0..2 {
                assert_eq!(m.member(values[0], set), Ok(true));
            }
            assert!(m.memo.bytes() <= 1 << 20);
        }
        assert!(indexed(&m) > 0);
        // More set keys than slots force replacement. Recompute every answer.
        for (set, query) in sets {
            assert_eq!(m.member(query, set), Ok(true));
            let miss = m.heap.tuple(&[Handle::int(-1).unwrap()]);
            assert_eq!(m.member(miss, set), Ok(false));
        }
        assert!(m.memo.bytes() <= 1 << 20);
        m.memo.clear();
        assert_eq!(m.memo.bytes(), 0);
        assert!(!m.heap.poisoned());
    }

    #[test]
    fn scalar_queries_keep_the_original_search_without_index_allocation() {
        let mut m = machine(false, true);
        let values: Vec<_> = (0..32).map(|i| Handle::int(i).unwrap()).collect();
        let set = m.heap.local_literal(&values).unwrap();
        let set = m.heap.norm(set).unwrap();
        for i in -1..33 {
            let query = Handle::int(i).unwrap();
            for _ in 0..3 {
                assert_eq!(
                    m.member(query, set),
                    m.heap.literal_member(query, set).unwrap()
                );
            }
        }
        assert_eq!(m.memo.member_index.bytes(), 0);
    }

    #[test]
    fn finite_fuel_retains_positive_and_exhausted_old_paths_without_reservation() {
        for enabled in [false, true] {
            let mut m = machine(false, true);
            m.opts.member_index = enabled;
            let set = tuple_set(&mut m);
            let query = m.heap.view(set).data[0];
            m.set_fuel(10000);
            for _ in 0..3 {
                assert_eq!(m.member(query, set), Ok(true));
            }
            assert_eq!(m.memo.member_index.cap, 0);
            assert_eq!(m.memo.member_index.bytes(), 0);
            let values: Vec<_> = (0..32).map(|i| Handle::int(i).unwrap()).collect();
            let raw = m.heap.local_literal(&values).unwrap();
            m.set_fuel(0);
            let error = m.member(values[0], raw).unwrap_err();
            assert_eq!(error.kind(), crate::ErrorKind::Unsupported);
            assert_eq!(error.message(), "the constant-folding budget");
            assert!(!m.heap.work_interrupted());
            assert_eq!(m.memo.member_index.cap, 0);
            m.set_fuel(u64::MAX);
            assert_eq!(m.memo.member_index.cap != 0, enabled);
            assert_eq!(m.charge(1_000_000), Ok(()));
            assert_eq!(m.fuel, u64::MAX);
            for _ in 0..2 {
                assert_eq!(m.member(query, set), Ok(true));
            }
            assert_eq!(indexed(&m), usize::from(enabled));
        }
    }

    #[derive(Debug)]
    struct Stop;
    impl crate::eval::WorkControl for Stop {
        fn proceed(&self) -> bool {
            false
        }
    }

    #[test]
    fn index_build_uses_the_actual_work_owner() {
        let mut m = machine(false, true);
        let set = tuple_set(&mut m);
        let query = m.heap.view(set).data[0];
        assert_eq!(m.member(query, set), Ok(true));
        m.set_work_control(Some(Arc::new(Stop)));
        let result = m.member(query, set);
        assert!(matches!(result, Err(e) if e.kind() == crate::ErrorKind::Stop));
        assert!(m.heap.work_interrupted());
        assert_eq!(indexed(&m), 0);
    }
}
