//! Value operations over both regions: views, hashing (`03-value-model.md`
//! §5), equality, the canonical order (§7), builders, materialization of
//! symbolic sets, and interning by deep close (§3.3).

use std::cmp::Ordering;
use std::sync::Arc;

use crate::Error;
use crate::arena::{Arena, Mark};
use crate::handle::{Handle, K_FCN, K_SET, K_SYM, Tag, fin, int_hash, mix2, mix64, scalar_hash};
use crate::node::{Kind, View, flags};
use crate::store::Store;

/// The largest set the core materializes.
pub const MAX_MATERIALIZE: usize = 1 << 24;

static EMPTY_DATA: [Handle; 0] = [];

/// A worker's view of the value regions: the shared persistent store and its
/// own arena.
#[derive(Debug)]
pub struct Heap {
    pub(crate) store: Arc<Store>,
    pub(crate) arena: Arena,
    poisoned: std::cell::Cell<bool>,
    /// `-delta-hash`: recompute hashes in full instead of incrementally.
    pub(crate) full_hash: bool,
    /// The integer semantics (`int.rs`).
    pub(crate) int_mode: crate::int::IntMode,
    /// Interned enumerations of persistent finite symbolic sets, for
    /// [`Heap::materialize_cached`].
    materialized: std::collections::HashMap<Handle, Handle>,
    history_control: Option<Arc<dyn crate::eval::WorkControl>>,
    history_steps: std::cell::Cell<u32>,
    work_interrupted: std::cell::Cell<bool>,
    /// This heap's copy of the string table's texts, by id, extended on a
    /// miss: string comparisons read it without the table's lock, which
    /// every worker would otherwise share (#550). Ids never change their
    /// text, so a copy never goes stale.
    texts: std::cell::RefCell<Vec<Arc<str>>>,
    /// This heap's view of the store's deduplication tables, for interning
    /// a value already present without the shard lock (#550).
    cache: crate::store::InternCache,
}

impl Drop for Heap {
    fn drop(&mut self) {
        self.store.flush_hits(&mut self.cache);
    }
}

/// The closure of a lazy node ([`Heap::closure`]).
#[derive(Clone, Copy, Debug)]
pub struct Closure<'a> {
    /// `Lambda` or `SetPred`.
    pub kind: Kind,
    /// The evaluator's closure id.
    pub id: u32,
    /// The function's domain, or the predicate's base set.
    pub domain: Handle,
    /// The captured values, in the evaluator's order.
    pub captures: &'a [Handle],
}

/// The hash of a set with the given element-hash sum.
#[must_use]
pub fn set_hash(len: usize, sum: u64) -> u64 {
    fin(K_SET, len as u64, sum)
}

/// The hash of a function with the given entry sum.
#[must_use]
pub fn fcn_hash(len: usize, sum: u64) -> u64 {
    fin(K_FCN, len as u64, sum)
}

impl Heap {
    /// A heap over `store` with a fresh arena.
    #[must_use]
    pub fn new(store: Arc<Store>, heap_arena: bool, full_hash: bool) -> Self {
        let int_mode = store.int_mode();
        Self {
            store,
            arena: Arena::new(heap_arena),
            poisoned: std::cell::Cell::new(false),
            full_hash,
            int_mode,
            materialized: std::collections::HashMap::new(),
            history_control: None,
            history_steps: std::cell::Cell::new(0),
            work_interrupted: std::cell::Cell::new(false),
            texts: std::cell::RefCell::new(Vec::new()),
            cache: crate::store::InternCache::default(),
        }
    }

    /// Compares two string or model-value ids in the table's order, as
    /// [`crate::strings::Strings::cmp`] does, from this heap's copy of the
    /// texts.
    pub(crate) fn str_cmp(&self, a: u32, b: u32) -> Ordering {
        if a == b {
            return Ordering::Equal;
        }
        let strings = self.store.strings();
        if strings.order() == crate::strings::StringOrder::Token {
            return a.cmp(&b);
        }
        let mut texts = self.texts.borrow_mut();
        if a.max(b) as usize >= texts.len() {
            let more = strings.texts_from(texts.len());
            texts.extend(more);
        }
        match (texts.get(a as usize), texts.get(b as usize)) {
            (Some(x), Some(y)) => x.cmp(y),
            _ => a.cmp(&b),
        }
    }

    pub(crate) fn known_name(&self, id: u32) -> bool {
        let mut texts = self.texts.borrow_mut();
        if id as usize >= texts.len() {
            let more = self.store.strings().texts_from(texts.len());
            texts.extend(more);
        }
        (id as usize) < texts.len()
    }

    /// TLC's model-value type for a name, using this heap's append-only
    /// text copy. The rule counts UTF-16 units, as `ModelValue` does.
    pub(crate) fn model_type(&self, id: u32) -> Option<u16> {
        let mut texts = self.texts.borrow_mut();
        if id as usize >= texts.len() {
            let more = self.store.strings().texts_from(texts.len());
            texts.extend(more);
        }
        let mut units = texts.get(id as usize)?.encode_utf16();
        let (first, second) = (units.next()?, units.next()?);
        (second == u16::from(b'_') && units.next().is_some()).then_some(first)
    }

    /// The store.
    #[must_use]
    pub fn store(&self) -> &Arc<Store> {
        &self.store
    }

    /// The arena.
    #[must_use]
    pub fn arena(&self) -> &Arena {
        &self.arena
    }

    /// Frees the transients of the current expansion.
    pub fn reset(&mut self) {
        self.arena.reset();
        self.store.flush_hits(&mut self.cache);
    }

    /// The current transient allocation point (§11.5 rule 2 of
    /// `03-value-model.md`).
    #[must_use]
    pub fn mark(&self) -> Mark {
        self.arena.mark()
    }

    /// Frees every transient allocated after `mark`. No handle allocated
    /// after the mark may be used afterwards, including one held in a frame
    /// slot or a thunk cache; such a use is detected as stale.
    pub fn rollback(&mut self, mark: Mark) {
        self.arena.rollback(mark);
    }

    /// Whether an invalid handle read was detected: an internal error.
    ///
    /// Persistent handles are coordinates within one [`Store`]. Callers must
    /// keep them with their issuing `Store`; equal coordinates from another `Store` cannot
    /// be distinguished from a locally issued handle by this check.
    #[must_use]
    pub fn poisoned(&self) -> bool {
        self.poisoned.get() || self.arena.stale() || self.store.poisoned()
    }

    /// The view of a compound handle. A stale or scalar handle gives an empty
    /// set view and poisons the heap.
    #[must_use]
    #[inline]
    pub fn view(&self, h: Handle) -> View<'_> {
        if h.is_persistent() {
            return self.store.view(h);
        }
        if h.is_transient()
            && let Some(v) = self.arena.lookup(h)
        {
            return v;
        }
        self.poison_view()
    }

    #[cold]
    fn poison_view(&self) -> View<'_> {
        self.poisoned.set(true);
        View {
            kind: Kind::Set,
            lo: 0,
            hash: 0,
            sum: 0,
            flags: 0,
            data: &EMPTY_DATA,
        }
    }

    /// Records an internal error: a lazy value reached an operation that
    /// needs a closed one (`03-value-model.md` §11.3).
    #[cold]
    fn lazy_misuse(&self) {
        self.poisoned.set(true);
    }

    /// The hash and flags of a child, for builders: no closedness check.
    #[inline]
    fn child(&self, h: Handle) -> (u64, u8) {
        if let Some(s) = scalar_hash(h) {
            return (s, 0);
        }
        let v = self.view(h);
        let hash = if self.full_hash {
            self.deep_hash(h)
        } else {
            v.hash
        };
        (
            hash,
            (v.flags & flags::LAZY)
                | if v.flags & (flags::HISTORY | flags::NESTED_VIEW) != 0 {
                    flags::NESTED_VIEW
                } else {
                    0
                },
        )
    }

    /// Whether a value must be closed by the evaluator before it is hashed,
    /// compared or interned (§11.3).
    #[must_use]
    pub fn is_lazy(&self, h: Handle) -> bool {
        h.is_compound() && self.view(h).is_lazy()
    }

    /// The kind of a compound, `None` for a scalar.
    #[must_use]
    pub fn kind(&self, h: Handle) -> Option<Kind> {
        h.is_compound().then(|| self.view(h).kind)
    }

    /// The value hash of a closed value. Finite symbolic sets must be
    /// normalized first. A lazy value poisons the heap.
    #[must_use]
    pub fn hash(&self, h: Handle) -> u64 {
        if let Some(s) = scalar_hash(h) {
            return s;
        }
        let v = self.view(h);
        if v.is_lazy() {
            self.lazy_misuse();
        }
        if self.full_hash {
            return self.deep_hash(h);
        }
        v.hash
    }

    /// The hash recomputed from the leaves, ignoring cached child hashes
    /// (the `-delta-hash` ablation).
    #[must_use]
    pub fn deep_hash(&self, h: Handle) -> u64 {
        if let Some(s) = scalar_hash(h) {
            return s;
        }
        let v = self.view(h);
        match v.kind {
            Kind::Set => {
                let sum = v
                    .data
                    .iter()
                    .fold(0u64, |a, &e| a.wrapping_add(self.deep_hash(e)));
                set_hash(v.data.len(), sum)
            }
            Kind::IntFcn => {
                let mut sum = 0u64;
                for (i, &x) in v.data.iter().enumerate() {
                    #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
                    let k = v.lo + i as i64;
                    sum = sum.wrapping_add(mix2(int_hash(k), self.deep_hash(x)));
                }
                fcn_hash(v.data.len(), sum)
            }
            Kind::Fcn => {
                let mut sum = 0u64;
                for (&k, &x) in v.keys().iter().zip(v.values()) {
                    sum = sum.wrapping_add(mix2(self.deep_hash(k), self.deep_hash(x)));
                }
                fcn_hash(v.len(), sum)
            }
            _ => v.hash,
        }
    }

    // ------------------------------------------------------------------
    // Equality and order
    // ------------------------------------------------------------------

    /// Value equality of two normalized handles.
    #[must_use]
    pub fn eq(&self, a: Handle, b: Handle) -> bool {
        if a == b {
            return true;
        }
        if !a.is_compound() || !b.is_compound() {
            return false;
        }
        if a.is_persistent()
            && b.is_persistent()
            && self.store.hashcons()
            && !a.is_lazy_persistent()
            && !b.is_lazy_persistent()
        {
            return false;
        }
        let (va, vb) = (self.view(a), self.view(b));
        if va.is_lazy() || vb.is_lazy() {
            self.lazy_misuse();
            return false;
        }
        if va.hash != vb.hash
            || va.kind != vb.kind
            || (va.kind != Kind::Set && va.lo != vb.lo)
            || va.data.len() != vb.data.len()
        {
            return false;
        }
        va.data.iter().zip(vb.data).all(|(&x, &y)| self.eq(x, y))
    }

    /// The rank of a handle's class in the canonical order (D2): model
    /// values, booleans, integers, strings, then compounds.
    fn scalar_rank(h: Handle) -> u8 {
        match h.tag() {
            Tag::Empty => 0,
            Tag::Model => 1,
            Tag::Bool => 2,
            Tag::Int => 3,
            Tag::Str => 4,
            Tag::Persistent | Tag::Transient => 5,
        }
    }

    fn kind_rank(k: Kind) -> u8 {
        match k {
            Kind::Set => 4,
            Kind::IntFcn | Kind::Fcn => 5,
            _ => 6,
        }
    }

    /// The canonical total order (D-010): model values by name, booleans,
    /// integers, strings by text, finite sets by size then elements,
    /// functions by size, domain then values, then symbolic sets
    /// structurally. Consistent with
    /// [`Heap::eq`] on normalized handles.
    #[must_use]
    pub fn cmp(&self, a: Handle, b: Handle) -> Ordering {
        if a == b {
            return Ordering::Equal;
        }
        let (mut ra, mut rb) = (Self::scalar_rank(a), Self::scalar_rank(b));
        if ra != rb {
            // A big integer is a compound that ranks among the integers.
            if a.is_compound() && self.view(a).kind == Kind::BigInt {
                ra = 3;
            }
            if b.is_compound() && self.view(b).kind == Kind::BigInt {
                rb = 3;
            }
            if ra != rb {
                return ra.cmp(&rb);
            }
            return self.int_cmp(a, b).unwrap_or(Ordering::Equal);
        }
        match a.tag() {
            Tag::Bool => a.as_bool().cmp(&b.as_bool()),
            Tag::Int => a.as_int().cmp(&b.as_int()),
            Tag::Str => match (a.as_str_id(), b.as_str_id()) {
                (Some(x), Some(y)) => self.str_cmp(x, y),
                _ => Ordering::Equal,
            },
            // By name, as strings (D2).
            Tag::Model => match (a.as_model_id(), b.as_model_id()) {
                (Some(x), Some(y)) => self.str_cmp(x, y),
                _ => Ordering::Equal,
            },
            Tag::Persistent | Tag::Transient => self.cmp_compound(a, b),
            Tag::Empty => Ordering::Equal,
        }
    }

    fn cmp_compound(&self, a: Handle, b: Handle) -> Ordering {
        let (va, vb) = (self.view(a), self.view(b));
        if va.is_lazy() || vb.is_lazy() {
            self.lazy_misuse();
            return Ordering::Equal;
        }
        if va.kind == Kind::BigInt || vb.kind == Kind::BigInt {
            return match (va.kind == Kind::BigInt, vb.kind == Kind::BigInt) {
                (true, true) => self.int_cmp(a, b).unwrap_or(Ordering::Equal),
                (true, false) => Ordering::Less,
                _ => Ordering::Greater,
            };
        }
        let (ra, rb) = (Self::kind_rank(va.kind), Self::kind_rank(vb.kind));
        if ra != rb {
            return ra.cmp(&rb);
        }
        match (va.kind, vb.kind) {
            (Kind::Set, Kind::Set) => va.data.len().cmp(&vb.data.len()).then_with(|| {
                for (&x, &y) in va.data.iter().zip(vb.data) {
                    let c = self.cmp(x, y);
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                Ordering::Equal
            }),
            // Two tuples, or integer-domain functions with the same domain:
            // the domains are equal, so only the values decide.
            (Kind::IntFcn, Kind::IntFcn) if va.lo == vb.lo && va.data.len() == vb.data.len() => {
                for (&x, &y) in va.data.iter().zip(vb.data) {
                    let c = self.cmp(x, y);
                    if c != Ordering::Equal {
                        return c;
                    }
                }
                Ordering::Equal
            }
            (ka, kb) if ka.is_fcn() && kb.is_fcn() => {
                let n = va.len();
                n.cmp(&vb.len()).then_with(|| {
                    for i in 0..n {
                        let c = self.cmp(fcn_key(&va, i), fcn_key(&vb, i));
                        if c != Ordering::Equal {
                            return c;
                        }
                    }
                    for (&x, &y) in va.values().iter().zip(vb.values()) {
                        let c = self.cmp(x, y);
                        if c != Ordering::Equal {
                            return c;
                        }
                    }
                    Ordering::Equal
                })
            }
            (ka, kb) => (ka as u8)
                .cmp(&(kb as u8))
                .then(va.lo.cmp(&vb.lo))
                .then(va.data.len().cmp(&vb.data.len()))
                .then_with(|| {
                    for (&x, &y) in va.data.iter().zip(vb.data) {
                        let c = self.cmp(x, y);
                        if c != Ordering::Equal {
                            return c;
                        }
                    }
                    Ordering::Equal
                }),
        }
    }

    // ------------------------------------------------------------------
    // Builders
    // ------------------------------------------------------------------

    /// A set from normalized elements in any order, with duplicates.
    pub fn set_from(&mut self, elems: &mut Vec<Handle>) -> Handle {
        if elems.len() > 1 {
            elems.sort_unstable_by(|&a, &b| self.cmp(a, b));
            elems.dedup_by(|a, b| self.eq(*a, *b));
        }
        self.set_sorted(elems)
    }

    /// A set from sorted, distinct, closed elements. A lazy element poisons
    /// the heap (§11.3).
    pub fn set_sorted(&mut self, elems: &[Handle]) -> Handle {
        let mut sum = 0u64;
        let mut lazy = 0;
        for &e in elems {
            let (h, f) = self.child(e);
            sum = sum.wrapping_add(h);
            lazy |= f;
        }
        if lazy & flags::LAZY != 0 {
            self.lazy_misuse();
        }
        self.arena
            .alloc(Kind::Set, 0, 0, set_hash(elems.len(), sum), sum, elems)
    }

    /// The empty set.
    pub fn empty_set(&mut self) -> Handle {
        self.set_sorted(&[])
    }

    /// A function over `lo..lo+n-1` from normalized values.
    pub fn int_fcn(&mut self, lo: i64, vals: &[Handle]) -> Handle {
        self.int_fcn_as(lo, vals, 0)
    }

    fn int_fcn_as(&mut self, lo: i64, vals: &[Handle], view: u8) -> Handle {
        let lo = if vals.is_empty() { 1 } else { lo };
        let mut sum = 0u64;
        let mut fl = 0;
        for (i, &v) in vals.iter().enumerate() {
            #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
            let k = lo + i as i64;
            let (h, f) = self.child(v);
            sum = sum.wrapping_add(mix2(int_hash(k), h));
            fl |= f;
        }
        self.arena.alloc(
            Kind::IntFcn,
            fl | view,
            lo,
            fcn_hash(vals.len(), sum),
            sum,
            vals,
        )
    }

    /// A tuple (a function over `1..n`).
    pub fn tuple(&mut self, vals: &[Handle]) -> Handle {
        self.int_fcn(1, vals)
    }

    /// A tuple literal `<<v1, ...>>`: the tuple [`Heap::tuple`] builds, with
    /// the view bit [`flags::TUPLE`] (as [`Heap::record`]).
    pub fn tuple_literal(&mut self, vals: &[Handle]) -> Handle {
        self.int_fcn_as(1, vals, flags::TUPLE)
    }

    /// A function from sorted, distinct, closed keys and their values (which
    /// may be lazy), in canonical form.
    pub fn fcn_sorted(&mut self, keys: &[Handle], vals: &[Handle]) -> Handle {
        self.fcn_sorted_as(keys, vals, 0)
    }

    fn fcn_sorted_as(&mut self, keys: &[Handle], vals: &[Handle], view: u8) -> Handle {
        if let Some(lo) = contiguous_ints(keys) {
            return self.int_fcn(lo, vals);
        }
        let mut sum = 0u64;
        let mut fl = 0;
        for (&k, &v) in keys.iter().zip(vals) {
            let (kh, kf) = self.child(k);
            let (vh, vf) = self.child(v);
            if kf != 0 {
                self.lazy_misuse();
            }
            sum = sum.wrapping_add(mix2(kh, vh));
            fl |= vf;
        }
        let mut data = Vec::with_capacity(keys.len() * 2);
        data.extend_from_slice(keys);
        data.extend_from_slice(vals);
        self.arena.alloc(
            Kind::Fcn,
            fl | view,
            0,
            fcn_hash(keys.len(), sum),
            sum,
            &data,
        )
    }

    /// A function from `(key, value)` pairs with distinct keys, any order.
    pub fn fcn_from_pairs(&mut self, pairs: &mut [(Handle, Handle)]) -> Handle {
        pairs.sort_unstable_by(|a, b| self.cmp(a.0, b.0));
        let keys: Vec<Handle> = pairs.iter().map(|p| p.0).collect();
        let vals: Vec<Handle> = pairs.iter().map(|p| p.1).collect();
        self.fcn_sorted(&keys, &vals)
    }

    /// A record literal `[f1 |-> v1, ...]` from sorted, distinct string keys:
    /// the function [`Heap::fcn_sorted`] builds, with [`flags::RECORD`]. The
    /// bit is a view bit: equality, hashing, order and [`Heap::intern`]
    /// ignore it, and [`Heap::intern_view`] keeps it.
    pub fn record(&mut self, keys: &[Handle], vals: &[Handle]) -> Handle {
        self.fcn_sorted_as(keys, vals, flags::RECORD)
    }

    /// A set enumeration `{e1, ...}` as TLC builds it before normalizing:
    /// the normalized, closed elements in construction order, duplicates kept, as a
    /// [`Kind::SetEnum`] view with [`flags::UNNORM`] (C1). It is normalized,
    /// hashed and interned as the set of its elements; only membership and
    /// error paths that the contract names read its order.
    pub fn set_enum(&mut self, elems: &[Handle]) -> Handle {
        self.symbolic(Kind::SetEnum, 0, elems)
    }

    /// A selected local literal, with a fresh arena-owned history identity.
    pub(crate) fn local_literal(&mut self, elems: &[Handle]) -> Result<Handle, Error> {
        if !elems
            .iter()
            .all(|&e| crate::compare::literal_value(self, e))
        {
            return Err(Error::unsupported(
                "a local literal outside the closed Bool/Int/tuple lane",
            ));
        }
        let slot = i64::try_from(self.arena.literal_sets.len())
            .ok()
            .and_then(|index| index.checked_add(1))
            .ok_or_else(|| Error::unsupported("local literal construction history capacity"))?;
        // SetEnum's lo payload is otherwise unused. A slot alone supplies
        // no authority: every lookup also checks this exact arena handle.
        self.history_poll()?;
        self.arena
            .literal_sets
            .try_reserve(1)
            .map_err(|_| Error::unsupported("construction history allocation"))?;
        // This is an extensional projection, not a normalization demand. It
        // cannot mutate this object's raw phase or decide a checked error.
        let mut projection = self.history_copy(elems)?;
        let identity = self.set_from(&mut projection);
        let view = self.view(identity);
        let handle = self.arena.alloc(
            Kind::SetEnum,
            flags::UNNORM | flags::HISTORY,
            slot,
            view.hash,
            view.sum,
            elems,
        );
        self.arena.literal_sets.push(crate::arena::LiteralSet {
            handle,
            phase: crate::arena::LiteralPhase::Raw,
            normalized: Handle::EMPTY,
            origin: 0,
            snapshots: [Handle::EMPTY; 2],
        });
        Ok(handle)
    }

    fn local_literal_index(&self, h: Handle) -> Option<usize> {
        if self.arena.literal_sets.is_empty() || !h.is_transient() {
            return None;
        }
        let view = self.arena.lookup(h)?;
        if !matches!(view.kind, Kind::SetEnum | Kind::Set) {
            return None;
        }
        let index = usize::try_from(view.lo).ok()?.checked_sub(1)?;
        self.arena
            .literal_sets
            .get(index)
            .filter(|entry| entry.handle == h || entry.normalized == h)
            .map(|_| index)
    }

    #[cfg_attr(
        not(test),
        allow(
            clippy::unused_self,
            reason = "the consumed-operation trace is arena-owned and test-only"
        )
    )]
    pub(crate) fn literal_step(&mut self, operation: &'static str, a: Handle, b: Handle) {
        #[cfg(test)]
        self.arena.literal_trace.push((operation, a, b));
        #[cfg(not(test))]
        let _ = (operation, a, b);
    }

    fn normalize_literal(&mut self, index: usize) -> Result<Handle, Error> {
        use crate::arena::LiteralPhase;
        match &self.arena.literal_sets[index].phase {
            LiteralPhase::Normalized(elems) => {
                let cached = self.arena.literal_sets[index].normalized;
                if self.arena.index(cached).is_some() || cached.is_persistent() {
                    return Ok(cached);
                }
                // The logical normalization predates rollback. Its owned array
                // survives, while a projection allocated after the mark does not.
                let mut projection = self.history_copy(elems)?;
                let identity = self.set_from(&mut projection);
                let view = self.view(identity);
                let slot = self.view(self.arena.literal_sets[index].handle).lo;
                let restored = self.arena.alloc(
                    Kind::Set,
                    flags::HISTORY,
                    slot,
                    view.hash,
                    view.sum,
                    &projection,
                );
                self.arena.literal_sets[index].normalized = restored;
                return Ok(restored);
            }
            LiteralPhase::Refused => {
                return Err(Error::unsupported(
                    "a previously refused local literal comparison",
                ));
            }
            LiteralPhase::Raw => {}
        }
        let raw = self.history_copy(self.view(self.arena.literal_sets[index].handle).data)?;
        let mut sorted = Vec::new();
        sorted
            .try_reserve(raw.len())
            .map_err(|_| Error::unsupported("construction normalization allocation"))?;
        for value in raw {
            self.history_poll()?;
            let (mut low, mut high) = (0, sorted.len());
            let mut duplicate = false;
            while low < high {
                let pivot = low + ((high - low) >> 1);
                let order = match crate::compare::literal_cmp(self, value, sorted[pivot], "insert")
                {
                    Ok(order) => order,
                    Err(error) => {
                        self.arena.literal_sets[index].phase = LiteralPhase::Refused;
                        return Err(error);
                    }
                };
                if order.is_eq() {
                    duplicate = true;
                    break;
                }
                if order.is_lt() {
                    high = pivot;
                } else {
                    low = pivot + 1;
                }
            }
            if !duplicate {
                sorted.insert(low, value);
            }
        }
        // The checked TLC array owns history and representatives. Its order
        // is never assumed to certify the separate canonical projection.
        let mut projection = self.history_copy(&sorted)?;
        let identity = self.set_from(&mut projection);
        let view = self.view(identity);
        let slot = self.view(self.arena.literal_sets[index].handle).lo;
        let normalized = self.arena.alloc(
            Kind::Set,
            flags::HISTORY,
            slot,
            view.hash,
            view.sum,
            &projection,
        );
        self.arena.literal_sets[index].normalized = normalized;
        self.arena.literal_sets[index].phase = LiteralPhase::Normalized(projection.into());
        Ok(normalized)
    }

    /// Executes the actual raw or normalized search for a covered literal.
    #[cfg(test)]
    pub(crate) fn literal_member(
        &mut self,
        query: Handle,
        set: Handle,
    ) -> Option<Result<bool, Error>> {
        self.construction_member(query, set, self.store.semantics().tlc_comparisons, None)
    }

    pub(crate) fn literal_member_indexed(
        &mut self,
        query: Handle,
        set: Handle,
        index: Option<&mut crate::member_index::MemberIndex>,
    ) -> Option<Result<bool, Error>> {
        self.construction_member(query, set, self.store.semantics().tlc_comparisons, index)
    }

    fn construction_member(
        &mut self,
        query: Handle,
        set: Handle,
        tlc: bool,
        member_index: Option<&mut crate::member_index::MemberIndex>,
    ) -> Option<Result<bool, Error>> {
        let index = self.local_literal_index(set);
        let retained = set.is_compound()
            && self.view(set).flags & flags::HISTORY != 0
            && self.view(set).kind == Kind::Set
            && self.store.owns_history(set);
        if index.is_none() && !retained {
            return None;
        }
        Some(self.construction_member_owned(query, set, index, tlc, member_index))
    }

    // Only the checked entrypoint or a Cup whose exact receiver was validated
    // calls this loop. The private arena index remains stable across RHS norm.
    fn construction_member_owned(
        &mut self,
        query: Handle,
        set: Handle,
        index: Option<usize>,
        tlc: bool,
        member_index: Option<&mut crate::member_index::MemberIndex>,
    ) -> Result<bool, Error> {
        use crate::arena::LiteralPhase;
        if self.construction_len(index, set) == 0 {
            return Ok(false);
        }
        if query
            .as_model_id()
            .or_else(|| query.as_str_id())
            .is_some_and(|id| !self.known_name(id))
        {
            return Err(Error::unsupported("an unknown construction query name"));
        }
        let normalized = match index.map(|i| &self.arena.literal_sets[i].phase) {
            None | Some(LiteralPhase::Normalized(_)) => true,
            Some(LiteralPhase::Raw) => false,
            Some(LiteralPhase::Refused) => {
                return Err(Error::unsupported(
                    "a previously refused construction comparison",
                ));
            }
        };
        if normalized
            && let Some(index) = index
            && let Err(error) = self.normalize_literal(index)
        {
            return Err(error);
        }
        let count = self.construction_len(index, set);
        // D-009's structural tuple membership is unchanged. Scalar-class
        // mismatches are deferred until no extensional hit exists, as in
        // the reference's tla_cmp::member, then the checked search decides.
        if !tlc && !self.is_lazy(query) && !self.has_raw_construction(query) {
            match self.construction_tla_result(query, set, index, normalized, count, member_index) {
                Ok(Some(answer)) => return Ok(answer),
                Err(error) => return Err(error),
                Ok(None) => {}
            }
        }
        if let Some(answer) = self.construction_scalar_miss(query, set, index, count)? {
            return Ok(answer);
        }
        if !crate::compare::literal_query(self, query) {
            return Err(Error::unsupported(
                "a query outside the closed scalar/tuple-convertible construction lane",
            ));
        }
        let result = if normalized {
            let (mut low, mut high) = (0, count);
            let mut found = Ok(false);
            while low < high {
                let pivot = low + ((high - low) >> 1);
                match crate::compare::literal_cmp(
                    self,
                    query,
                    self.construction_at(index, set, pivot),
                    "search",
                ) {
                    Ok(order) if order.is_eq() => {
                        found = Ok(true);
                        break;
                    }
                    Ok(order) if order.is_lt() => high = pivot,
                    Ok(_) => low = pivot + 1,
                    Err(error) => {
                        found = Err(error);
                        break;
                    }
                }
            }
            found
        } else {
            let mut found = Ok(false);
            for position in 0..count {
                self.history_poll()?;
                let member = self.construction_at(index, set, position);
                match crate::compare::literal_equals(self, member, query) {
                    Ok(true) => {
                        found = Ok(true);
                        break;
                    }
                    Ok(false) => {}
                    Err(error) => {
                        found = Err(error);
                        break;
                    }
                }
            }
            found
        };
        if result.is_err()
            && let Some(index) = index
        {
            self.arena.literal_sets[index].phase = LiteralPhase::Refused;
        }
        result
    }

    fn construction_scalar_miss(
        &self,
        query: Handle,
        set: Handle,
        index: Option<usize>,
        count: usize,
    ) -> Result<Option<bool>, Error> {
        let first = self.construction_at(index, set, 0);
        let class = crate::compare::class(self, first);
        if !class.is_scalar()
            || crate::compare::class(self, query) == class
            || query.as_model_id().is_some()
        {
            return Ok(None);
        }
        // All possible pivots have the same primitive scalar receiver class.
        // Reuse the old typed error/refusal path; source replay owns its class.
        for i in 1..count {
            self.history_poll()?;
            if crate::compare::class(self, self.construction_at(index, set, i)) != class {
                return Ok(None);
            }
        }
        crate::compare::miss(self, query, &[first])?;
        Ok(Some(false))
    }

    fn construction_tla_result(
        &mut self,
        query: Handle,
        set: Handle,
        index: Option<usize>,
        normalized: bool,
        count: usize,
        member_index: Option<&mut crate::member_index::MemberIndex>,
    ) -> Result<Option<bool>, Error> {
        if normalized {
            let view = index.map_or(set, |i| self.arena.literal_sets[i].normalized);
            let values = self.view(view).data;
            let indexed = match member_index {
                Some(index) => index.search(self, set, query, values)?,
                None => None,
            };
            if indexed.unwrap_or_else(|| values.binary_search_by(|&e| self.cmp(e, query)).is_ok()) {
                return Ok(Some(true));
            }
            return Ok((!self.construction_sorted_needs_checked(query, values)?).then_some(false));
        }
        for i in 0..count {
            self.history_poll()?;
            if self.eq(query, self.construction_at(index, set, i)) {
                return Ok(Some(true));
            }
        }
        for i in 0..count {
            self.history_poll()?;
            let e = self.construction_at(index, set, i);
            if crate::compare::default_member_needs_checked(self, query, e) {
                return Ok(None);
            }
        }
        Ok(Some(false))
    }

    // This array is the canonical projection of an already proved normalized
    // construction. Its ordering supplies no phase or history authority. Model
    // values precede the primitive scalar classes, which precede compounds.
    // Only model names need individual inspection; primitive class clashes
    // are decided by the non-model endpoints, as in compare::miss_scalars.
    fn construction_sorted_needs_checked(
        &self,
        query: Handle,
        values: &[Handle],
    ) -> Result<bool, Error> {
        self.history_poll()?;
        let models = values.partition_point(|h| h.as_model_id().is_some());
        for &value in &values[..models] {
            self.history_poll()?;
            if crate::compare::default_member_needs_checked(self, query, value) {
                return Ok(true);
            }
        }
        let remaining = &values[models..];
        Ok(remaining
            .first()
            .is_some_and(|&first| crate::compare::default_member_needs_checked(self, query, first))
            || remaining.last().is_some_and(|&last| {
                crate::compare::default_member_needs_checked(self, query, last)
            }))
    }

    fn construction_len(&self, index: Option<usize>, set: Handle) -> usize {
        match index.map(|i| &self.arena.literal_sets[i].phase) {
            Some(crate::arena::LiteralPhase::Normalized(elems)) => elems.len(),
            _ => self.view(set).data.len(),
        }
    }

    fn construction_at(&self, index: Option<usize>, set: Handle, position: usize) -> Handle {
        match index.map(|i| &self.arena.literal_sets[i].phase) {
            Some(crate::arena::LiteralPhase::Normalized(elems)) => elems[position],
            _ => self.view(set).data[position],
        }
    }

    fn history_copy(&self, values: &[Handle]) -> Result<Vec<Handle>, Error> {
        self.history_poll()?;
        let mut out = Vec::new();
        out.try_reserve(values.len())
            .map_err(|_| Error::unsupported("construction view allocation"))?;
        for chunk in values.chunks(64) {
            self.history_poll()?;
            out.extend_from_slice(chunk);
        }
        Ok(out)
    }

    fn construction_elements(&self, h: Handle) -> Option<&[Handle]> {
        if let Some(index) = self.local_literal_index(h) {
            return match &self.arena.literal_sets[index].phase {
                crate::arena::LiteralPhase::Raw => Some(self.view(h).data),
                crate::arena::LiteralPhase::Normalized(elems) => Some(elems),
                crate::arena::LiteralPhase::Refused => None,
            };
        }
        (h.is_compound()
            && self.view(h).kind == Kind::Set
            && self.view(h).flags & flags::HISTORY != 0
            && self.store.owns_history(h))
        .then(|| self.view(h).data)
    }

    /// Exact reducible SetEnumValue.cup for actual owned constructions.
    pub(crate) fn construction_union(
        &mut self,
        a: Handle,
        b: Handle,
    ) -> Option<Result<Handle, Error>> {
        let receiver = self.construction_elements(a)?;
        // Empty receiver returns the actual RHS object without demanding it.
        if receiver.is_empty() {
            return Some(Ok(b));
        }
        self.construction_elements(b)?;
        // Capture the original receiver before RHS normalization can mutate an
        // alias. Eligibility itself borrows and copies nothing.
        let mut out = match self.history_copy(receiver) {
            Ok(out) => out,
            Err(error) => return Some(Err(error)),
        };
        Some((|| {
            let b = self.norm(b)?;
            let right_index = self.local_literal_index(b);
            let receiver_index = self.local_literal_index(a);
            let count = self.construction_len(right_index, b);
            out.try_reserve(count)
                .map_err(|_| Error::unsupported("UNION construction allocation"))?;
            for position in 0..count {
                self.history_poll()?;
                let value = self.construction_at(right_index, b, position);
                if !self.construction_member_owned(value, a, receiver_index, true, None)? {
                    out.push(value);
                }
            }
            self.local_literal(&out)
        })())
    }

    /// The same function with the entry at index `i` replaced, hashing in
    /// O(1) from the stored sum (S2).
    #[allow(
        clippy::many_single_char_names,
        reason = "short names for the hash algebra"
    )]
    pub fn fcn_with(&mut self, f: Handle, i: usize, new: Handle) -> Handle {
        let v = self.view(f);
        let (kind, lo, sum, n, was_lazy) = (v.kind, v.lo, v.sum, v.len(), v.is_lazy());
        // TLC's `RecordValue` and `TupleValue` EXCEPT keep their class.
        let record = v.flags & (flags::RECORD | flags::TUPLE);
        // Small updates need writable scratch, not a separately allocated
        // payload: the arena copies the completed slice below. General
        // functions have both keys and values, so eight entries need 16 slots.
        let mut small = [Handle::EMPTY; 16];
        let mut owned;
        let data = if n <= 8
            && i < n
            && (kind == Kind::IntFcn || (kind == Kind::Fcn && v.data.len() == n * 2))
        {
            let data = &mut small[..v.data.len()];
            data.copy_from_slice(v.data);
            data
        } else {
            owned = v.data.to_vec();
            owned.as_mut_slice()
        };
        let (key_hash, slot) = match kind {
            Kind::IntFcn => {
                #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
                let k = lo + i as i64;
                (int_hash(k), i)
            }
            _ => (self.child(data[i]).0, n + i),
        };
        let old = data[slot];
        data[slot] = new;
        // Recount laziness only when it may have changed; O(1) otherwise.
        let fl = if was_lazy {
            let vals = if kind == Kind::IntFcn {
                &data[..]
            } else {
                &data[n..]
            };
            vals.iter().fold(0, |a, &x| a | self.child(x).1)
        } else {
            (v.flags & flags::NESTED_VIEW) | self.child(new).1
        };
        let sum = if self.full_hash {
            // `-delta-hash`: recompute the sum from every entry.
            let mut s = 0u64;
            for j in 0..n {
                let (kh, x) = match kind {
                    Kind::IntFcn => {
                        #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
                        let k = lo + j as i64;
                        (int_hash(k), data[j])
                    }
                    _ => (self.child(data[j]).0, data[n + j]),
                };
                s = s.wrapping_add(mix2(kh, self.child(x).0));
            }
            s
        } else {
            sum.wrapping_sub(mix2(key_hash, self.child(old).0))
                .wrapping_add(mix2(key_hash, self.child(new).0))
        };
        self.arena
            .alloc(kind, fl | record, lo, fcn_hash(n, sum), sum, data)
    }

    /// A symbolic set node. Its hash is structural, not extensional: a
    /// finite symbolic set is normalized before it is hashed as a value.
    pub fn symbolic(&mut self, kind: Kind, lo: i64, data: &[Handle]) -> Handle {
        let mut h = mix64(K_SYM ^ (kind as u64) ^ lo.cast_unsigned().rotate_left(17));
        let mut fl = if kind.is_lazy() { flags::LAZY } else { 0 };
        for &d in data {
            let (dh, df) = self.child(d);
            h = mix2(h, dh);
            fl |= df;
        }
        if kind == Kind::SetEnum {
            if fl & flags::LAZY != 0 {
                self.lazy_misuse();
            }
            fl |= flags::UNNORM;
        }
        self.arena.alloc(kind, fl, lo, h, 0, data)
    }

    /// `[x \in domain |-> e]`, unmaterialized: the evaluator's closure
    /// `closure` over `captures` (§11.2). Lazy.
    pub fn lambda(&mut self, closure: u32, domain: Handle, captures: &[Handle]) -> Handle {
        self.closure_node(Kind::Lambda, closure, domain, captures)
    }

    /// `{x \in base : P}`, unenumerated: the evaluator's closure `closure`
    /// over `captures` (§11.2). Lazy.
    pub fn set_pred(&mut self, closure: u32, base: Handle, captures: &[Handle]) -> Handle {
        self.closure_node(Kind::SetPred, closure, base, captures)
    }

    fn closure_node(&mut self, kind: Kind, id: u32, first: Handle, captures: &[Handle]) -> Handle {
        let mut data = Vec::with_capacity(captures.len() + 1);
        data.push(first);
        data.extend_from_slice(captures);
        self.symbolic(kind, i64::from(id), &data)
    }

    /// `a \cup b`, kept symbolic (TLC's `SetCupValue`): membership is
    /// either side's, and it is enumerated only when normalized.
    pub fn union(&mut self, a: Handle, b: Handle) -> Handle {
        self.symbolic(Kind::Union, 0, &[a, b])
    }

    /// The closure of a `Lambda` or `SetPred` node.
    #[must_use]
    pub fn closure(&self, h: Handle) -> Option<Closure<'_>> {
        if !h.is_compound() {
            return None;
        }
        let v = self.view(h);
        if !v.kind.is_lazy() {
            return None;
        }
        let (&first, captures) = v.data.split_first()?;
        Some(Closure {
            kind: v.kind,
            id: u32::try_from(v.lo).ok()?,
            domain: first,
            captures,
        })
    }

    /// The same kind as `h` with new contents, in canonical form and
    /// rehashed: a set is re-sorted, a function keeps its keys and takes new
    /// values. The evaluator closes a lazy value bottom-up with it
    /// (§11.5 rule 4). For a `Fcn`, `data` is the keys then the values, as
    /// in its view.
    pub fn rebuild(&mut self, h: Handle, data: &[Handle]) -> Handle {
        if !h.is_compound() {
            return h;
        }
        let v = self.view(h);
        let (kind, lo, n) = (v.kind, v.lo, v.data.len());
        if data.len() != n {
            self.lazy_misuse();
            return h;
        }
        match kind {
            Kind::Set => self.set_from(&mut data.to_vec()),
            Kind::IntFcn => self.int_fcn(lo, data),
            Kind::Fcn => {
                let (keys, vals) = data.split_at(n / 2);
                self.fcn_sorted(keys, vals)
            }
            _ => self.symbolic(kind, lo, data),
        }
    }

    /// `lo..hi`.
    pub fn interval(&mut self, lo: i64, hi: i64) -> Handle {
        let hi_h = Handle::int(hi).unwrap_or(Handle::EMPTY);
        self.symbolic(Kind::Interval, lo, &[hi_h])
    }

    // ------------------------------------------------------------------
    // Normalization and materialization
    // ------------------------------------------------------------------

    /// Whether a set is finite (symbolic sets over finite components).
    #[must_use]
    pub fn is_finite(&self, h: Handle) -> bool {
        if !h.is_compound() {
            return true;
        }
        let v = self.view(h);
        match v.kind {
            Kind::Set
            | Kind::IntFcn
            | Kind::Fcn
            | Kind::Lambda
            | Kind::BigInt
            | Kind::Interval
            | Kind::SetEnum => true,
            Kind::Nat | Kind::Int => false,
            Kind::SeqSet => self.view_is_empty_set(v.data[0]),
            Kind::Subset | Kind::FcnSet | Kind::Product => {
                v.data.iter().all(|&d| self.is_finite(d))
            }
            Kind::RecSet => v.data[v.data.len() / 2..]
                .iter()
                .all(|&d| self.is_finite(d)),
            Kind::Union => v.data.iter().all(|&d| self.is_finite(d)),
            Kind::SetPred => self.is_finite(v.data[0]),
        }
    }

    fn view_is_empty_set(&self, h: Handle) -> bool {
        h.is_compound() && {
            let v = self.view(h);
            v.kind == Kind::Set && v.data.is_empty()
        }
    }

    /// Normalizes a value: a finite symbolic set becomes an enumerated set.
    /// Other values are returned unchanged.
    ///
    /// # Errors
    /// The set is too large to materialize.
    pub fn norm(&mut self, h: Handle) -> Result<Handle, Error> {
        if let Some(index) = self.local_literal_index(h) {
            return self.normalize_literal(index);
        }
        if !h.is_compound() {
            return Ok(h);
        }
        let v = self.view(h);
        if !v.kind.is_symbolic() || v.is_lazy() || !self.is_finite(h) {
            return Ok(h);
        }
        self.materialize(h)
    }

    /// The elements of a finite set in canonical order, as a new enumerated
    /// set.
    ///
    /// # Errors
    /// An infinite or too-large set.
    pub fn materialize(&mut self, h: Handle) -> Result<Handle, Error> {
        self.materialize_fresh(h)
    }

    /// As [`Heap::materialize`], but a persistent set's enumeration is
    /// interned and kept, so that it is built once per worker. Used only by
    /// membership under `-symbolic-member`: interning every enumeration
    /// would fill the store with values no state holds.
    ///
    /// # Errors
    /// As [`Heap::materialize`].
    pub fn materialize_cached(&mut self, h: Handle) -> Result<Handle, Error> {
        if h.is_persistent() {
            if let Some(&m) = self.materialized.get(&h) {
                return Ok(m);
            }
            let m = self.materialize_fresh(h)?;
            let m = self.intern(m)?;
            self.materialized.insert(h, m);
            return Ok(m);
        }
        self.materialize_fresh(h)
    }

    fn materialize_fresh(&mut self, h: Handle) -> Result<Handle, Error> {
        let mut cur = self.cursor(h)?;
        let mut out = Vec::new();
        while let Some(e) = cur.next(self)? {
            if out.len() >= MAX_MATERIALIZE {
                return Err(Error::unsupported(
                    "materializing a set of more than 2^24 elements",
                ));
            }
            out.push(e);
        }
        Ok(self.set_sorted(&out))
    }

    /// The elements of a finite set (normalized) as a vector.
    ///
    /// # Errors
    /// As [`Heap::materialize`].
    pub fn elements(&mut self, h: Handle) -> Result<Vec<Handle>, Error> {
        if h.is_compound() {
            let v = self.view(h);
            if v.kind == Kind::Set {
                return Ok(v.data.to_vec());
            }
        }
        let mut cur = self.cursor(h)?;
        let mut out = Vec::new();
        while let Some(e) = cur.next(self)? {
            if out.len() >= MAX_MATERIALIZE {
                return Err(Error::unsupported(
                    "enumerating a set of more than 2^24 elements",
                ));
            }
            out.push(e);
        }
        Ok(out)
    }

    /// A cursor over a set's elements in canonical order.
    ///
    /// # Errors
    /// The value is not an enumerable set.
    pub fn cursor(&mut self, h: Handle) -> Result<Cursor, Error> {
        if !h.is_compound() {
            return Err(Error::eval(
                "attempted to enumerate a value that is not a set",
            ));
        }
        let h = if self.local_literal_index(h).is_some() {
            self.norm(h)?
        } else {
            h
        };
        let v = self.view(h);
        let (kind, lo, data) = (v.kind, v.lo, v.data.to_vec());
        Ok(match kind {
            Kind::Set => Cursor::Array { set: h, i: 0 },
            Kind::Interval => Cursor::Range {
                cur: lo,
                hi: data[0].as_int().unwrap_or(lo - 1),
            },
            Kind::Product => {
                let comps = data
                    .iter()
                    .map(|&s| {
                        let s = self.norm(s)?;
                        self.elements(s)
                    })
                    .collect::<Result<Vec<_>, Error>>()?;
                Cursor::odometer(Shape::Tuple, comps)
            }
            Kind::FcnSet => {
                let dom = self.norm(data[0])?;
                let keys = self.elements(dom)?;
                let rng = self.norm(data[1])?;
                let r = self.elements(rng)?;
                let comps = vec![r; keys.len()];
                Cursor::odometer(Shape::Fcn(keys), comps)
            }
            Kind::RecSet => {
                let n = data.len() / 2;
                let names = data[..n].to_vec();
                let comps = data[n..]
                    .iter()
                    .map(|&s| {
                        let s = self.norm(s)?;
                        self.elements(s)
                    })
                    .collect::<Result<Vec<_>, Error>>()?;
                Cursor::odometer(Shape::Fcn(names), comps)
            }
            Kind::Subset => {
                let base = self.norm(data[0])?;
                let elems = self.elements(base)?;
                if elems.len() > 24 {
                    return Err(Error::unsupported(
                        "enumerating SUBSET of a set of more than 24 elements",
                    ));
                }
                Cursor::Subset {
                    elems,
                    k: 0,
                    combo: Vec::new(),
                    started: false,
                }
            }
            Kind::SetEnum => {
                let mut v = data;
                let set = self.set_from(&mut v);
                Cursor::Array { set, i: 0 }
            }
            Kind::Union => {
                let (a, b) = (self.norm(data[0])?, self.norm(data[1])?);
                let mut v = self.elements(a)?;
                v.extend(self.elements(b)?);
                let set = self.set_from(&mut v);
                Cursor::Array { set, i: 0 }
            }
            Kind::Nat | Kind::Int | Kind::SeqSet => {
                return Err(Error::eval("attempted to enumerate an infinite set"));
            }
            Kind::IntFcn | Kind::Fcn | Kind::Lambda => {
                return Err(Error::eval("attempted to enumerate a function as a set"));
            }
            Kind::BigInt => {
                return Err(Error::eval(
                    "attempted to enumerate a value that is not a set",
                ));
            }
            Kind::SetPred => {
                return Err(Error::internal(
                    "a set predicate reached the heap's enumeration; the evaluator closes it",
                ));
            }
        })
    }

    /// The number of elements of a finite set.
    ///
    /// # Errors
    /// Not a finite set.
    pub fn cardinality(&mut self, h: Handle) -> Result<u64, Error> {
        if !h.is_compound() {
            return Err(Error::eval("Cardinality of a value that is not a set"));
        }
        let v = self.view(h);
        if v.is_lazy() {
            return Err(Error::internal(
                "Cardinality of an unclosed lazy value; the evaluator closes it",
            ));
        }
        match v.kind {
            Kind::Set => Ok(v.data.len() as u64),
            #[allow(clippy::cast_sign_loss, reason = "checked non-negative")]
            Kind::Interval => {
                let hi = v.data[0].as_int().unwrap_or(v.lo - 1);
                Ok(if hi < v.lo { 0 } else { (hi - v.lo + 1) as u64 })
            }
            _ => {
                let m = self.norm(h)?;
                if m == h {
                    return Err(Error::eval("Cardinality of an infinite set"));
                }
                self.cardinality(m)
            }
        }
    }

    // ------------------------------------------------------------------
    // Interning
    // ------------------------------------------------------------------

    /// Interns a value into the persistent store, deep-closing it: children
    /// first, finite symbolic sets materialized.
    ///
    /// # Errors
    /// A set too large to materialize.
    pub fn intern(&mut self, h: Handle) -> Result<Handle, Error> {
        let h = if self.local_literal_index(h).is_some() {
            self.norm(h)?
        } else {
            h
        };
        match h.tag() {
            Tag::Transient => {}
            Tag::Persistent => {
                if h.is_lazy_persistent() {
                    let v = self.view(h);
                    if v.is_lazy() {
                        return Err(Error::internal("interning an unclosed lazy value"));
                    }
                    if !v.kind.is_symbolic() {
                        // A record literal's view (`intern_view`): its identity
                        // has no view bit.
                        let (kind, lo, hash, sum) = (
                            v.kind,
                            if v.kind == Kind::Set { 0 } else { v.lo },
                            v.hash,
                            v.sum,
                        );
                        let mut data = v.data.to_vec();
                        for d in &mut data {
                            *d = self.intern(*d)?;
                        }
                        return Ok(self.store.intern(
                            &mut self.cache,
                            kind,
                            0,
                            lo,
                            hash,
                            sum,
                            &data,
                        ));
                    }
                }
                let k = self.view(h).kind;
                if k.is_symbolic() && self.is_finite(h) {
                    let m = self.materialize(h)?;
                    return self.intern(m);
                }
                return Ok(h);
            }
            _ => return Ok(h),
        }
        let Some(idx) = self.arena.index(h) else {
            self.arena.mark_stale();
            return Ok(Handle::EMPTY);
        };
        let done = self.arena.interned[idx];
        if !done.is_empty() {
            return Ok(done);
        }
        let v = self.view(h);
        if v.is_lazy() {
            return Err(Error::internal(
                "interning an unclosed lazy value; the evaluator closes it first",
            ));
        }
        if v.kind.is_symbolic() && self.is_finite(h) {
            let m = self.materialize(h)?;
            let p = self.intern(m)?;
            self.arena.interned[idx] = p;
            return Ok(p);
        }
        let (kind, lo, hash, sum) = (
            v.kind,
            if v.kind == Kind::Set { 0 } else { v.lo },
            v.hash,
            v.sum,
        );
        if v.data.len() <= 8 && self.intern_children_unchanged(v.data) {
            // Reborrow only the arena so the ordinary store cache remains
            // independently mutable. No arena mutation occurred since view.
            let Some(v) = self.arena.lookup(h) else {
                self.arena.mark_stale();
                return Ok(Handle::EMPTY);
            };
            let p = self
                .store
                .intern(&mut self.cache, kind, 0, lo, hash, sum, v.data);
            self.arena.interned[idx] = p;
            return Ok(p);
        }
        let mut data = v.data.to_vec();
        for d in &mut data {
            if d.is_compound() {
                *d = self.intern(*d)?;
            }
        }
        let p = self
            .store
            .intern(&mut self.cache, kind, 0, lo, hash, sum, &data);
        self.arena.interned[idx] = p;
        Ok(p)
    }

    /// Whether each child takes the identical-handle return in `intern`.
    /// Stop at the first recursive case: reading a later child could move
    /// poisoning ahead of that child's error, materialization or owned stop.
    fn intern_children_unchanged(&self, data: &[Handle]) -> bool {
        data.iter().all(|&h| {
            !h.is_compound()
                || (h.is_persistent()
                    && !h.is_lazy_persistent()
                    && !self.view(h).kind.is_symbolic())
        })
    }

    /// Interns a constant, preserving available function literal classes
    /// recursively and keeping symbolic sets symbolic. This is a value used
    /// for evaluation, not its canonical identity (`0121` in the ADRs).
    /// A lazy constant (for example `[x \in Nat |-> x + 1]`) stays lazy: it
    /// is interned structurally, never compared by handle (§11.1).
    ///
    /// # Errors
    /// As [`Heap::intern`].
    pub fn intern_const(&mut self, h: Handle) -> Result<Handle, Error> {
        if self.local_literal_index(h).is_some() {
            return self.intern_exact(h);
        }
        if !h.is_transient() {
            return Ok(h);
        }
        self.intern_structural(h)
    }

    /// Interns a node as it is, view bits included, with its components
    /// interned as constants: apart from identities (class 3) when it is
    /// lazy or a view.
    fn intern_structural(&mut self, h: Handle) -> Result<Handle, Error> {
        let v = self.view(h);
        let (kind, mut fl, lo, hash, sum) = (v.kind, v.flags, v.lo, v.hash, v.sum);
        let mut data = v.data.to_vec();
        for d in &mut data {
            *d = self.intern_const(*d)?;
            if d.is_compound() && self.view(*d).flags & flags::VIEW != 0 {
                fl |= flags::NESTED_VIEW;
            }
        }
        Ok(self
            .store
            .intern(&mut self.cache, kind, fl, lo, hash, sum, &data))
    }

    /// Interns a closed value exactly as it is: every node keeps its kind
    /// and view bits (a symbolic set stays symbolic, a record literal stays
    /// one), so the persistent handle reads like `h` everywhere, and two
    /// handles it returns are equal exactly when the representations are.
    /// Step-predicate memo sites use it for their keys and results
    /// (`06-memoization.md` §2.3); graph capture uses it to retain original
    /// target representations independently of canonical state identities.
    ///
    /// # Errors
    /// `h` is lazy (holds a closure only the evaluator can force).
    pub fn intern_exact(&mut self, h: Handle) -> Result<Handle, Error> {
        if let Some(index) = self.local_literal_index(h) {
            return self.snapshot_history(index);
        }
        if !h.is_transient() {
            return Ok(h);
        }
        let v = self.view(h);
        if v.is_lazy() {
            return Err(Error::internal("interning an unclosed lazy value"));
        }
        let (kind, mut fl, lo, hash, sum) = (v.kind, v.flags, v.lo, v.hash, v.sum);
        let mut data = v.data.to_vec();
        for d in &mut data {
            if d.is_transient() {
                *d = self.intern_exact(*d)?;
            }
            if d.is_compound() && self.view(*d).flags & flags::VIEW != 0 {
                fl |= flags::NESTED_VIEW;
            }
        }
        Ok(self
            .store
            .intern(&mut self.cache, kind, fl, lo, hash, sum, &data))
    }

    /// Interns a state value with its lazy view (§8): the identity handle
    /// (as [`Heap::intern`]) and, when the value is a symbolic set or carries
    /// a view bit ([`flags::VIEW`]) at the top level, that representation
    /// interned apart from identities; otherwise [`Handle::EMPTY`]. Nested
    /// view bits are not kept. The engine keeps the view in a side table
    /// only where the plan asks for it; hashing and equality use the identity.
    ///
    /// # Errors
    /// As [`Heap::intern`].
    pub fn intern_view(&mut self, h: Handle) -> Result<(Handle, Handle), Error> {
        let view = if !h.is_compound() {
            Handle::EMPTY
        } else if self.view(h).kind.is_symbolic() {
            self.intern_const(h)?
        } else if self.view(h).flags & flags::VIEW != 0 {
            if h.is_transient() {
                self.intern_structural(h)?
            } else {
                h
            }
        } else {
            Handle::EMPTY
        };
        Ok((self.intern(h)?, view))
    }

    pub(crate) fn set_history_control(
        &mut self,
        control: Option<Arc<dyn crate::eval::WorkControl>>,
    ) {
        self.history_control = control;
    }

    pub(crate) fn mark_work_interrupted(&self) {
        self.work_interrupted.set(true);
    }

    /// Whether this machine reached its caller-owned cooperative stop.
    #[must_use]
    pub fn work_interrupted(&self) -> bool {
        self.work_interrupted.get()
    }

    pub(crate) fn history_poll(&self) -> Result<(), Error> {
        let steps = self.history_steps.get();
        self.history_steps.set(steps.wrapping_add(1));
        if steps.trailing_zeros() >= 6
            && self.history_control.as_ref().is_some_and(|c| !c.proceed())
        {
            self.mark_work_interrupted();
            return Err(Error::stop());
        }
        Ok(())
    }

    /// Whether this value retains an actual owned construction view. Flags
    /// are traversal hints; only a constructor/snapshot owns history authority.
    #[must_use]
    pub fn has_construction_history(&self, h: Handle) -> bool {
        if !h.is_compound() {
            return false;
        }
        let view = self.view(h);
        view.flags & flags::HISTORY != 0
            || (view.flags & flags::NESTED_VIEW != 0
                && view.data.iter().any(|&h| self.has_construction_history(h)))
    }

    pub(crate) fn has_raw_construction(&self, h: Handle) -> bool {
        if let Some(index) = self.local_literal_index(h) {
            return matches!(
                self.arena.literal_sets[index].phase,
                crate::arena::LiteralPhase::Raw
            );
        }
        if !h.is_compound() {
            return false;
        }
        let view = self.view(h);
        view.flags & flags::HISTORY != 0 && view.kind == Kind::SetEnum
            || view.flags & flags::NESTED_VIEW != 0
                && view.data.iter().any(|&h| self.has_raw_construction(h))
    }

    /// Performs actual fingerprint normalization while retaining proven views.
    /// Canonical state keys are obtained separately through `intern`.
    ///
    /// # Errors
    /// Checked normalization, allocation, or the owner's cooperative stop.
    pub fn normalize_construction(&mut self, h: Handle) -> Result<Handle, Error> {
        if !self.has_construction_history(h) {
            return Ok(h);
        }
        self.history_poll()?;
        if self.local_literal_index(h).is_some() {
            return self.norm(h);
        }
        let view = self.view(h);
        if view.flags & flags::HISTORY != 0 {
            return if self.store.owns_history(h) && view.kind == Kind::Set {
                Ok(h)
            } else {
                Err(Error::unsupported("unproved retained construction history"))
            };
        }
        let (kind, fl, lo, hash, sum) = (view.kind, view.flags, view.lo, view.hash, view.sum);
        let mut data = self.history_copy(view.data)?;
        for child in &mut data {
            *child = self.normalize_construction(*child)?;
        }
        Ok(self.arena.alloc(kind, fl, lo, hash, sum, &data))
    }

    /// D-009's quiet, extensional equality on affected children. Like Value's
    /// `semantic_eq`, pointer/hash/domain checks precede reverse field demand.
    /// Actual constructor objects retain their phase.
    pub(crate) fn quiet_construction_equals(
        &mut self,
        a: Handle,
        b: Handle,
    ) -> Result<bool, Error> {
        self.history_poll()?;
        if a == b {
            return Ok(true);
        }
        if crate::compare::class(self, a) != crate::compare::class(self, b)
            || self.hash(a) != self.hash(b)
        {
            return Ok(false);
        }
        if self.local_literal_index(a).is_some() || self.local_literal_index(b).is_some() {
            let a = self.quiet_literal(a)?;
            let b = self.quiet_literal(b)?;
            return Ok(self.eq(a, b));
        }
        if !self.has_raw_construction(a) && !self.has_raw_construction(b) {
            return Ok(self.eq(a, b));
        }
        let (va, vb) = (self.view(a), self.view(b));
        if va.kind != Kind::IntFcn || vb.kind != Kind::IntFcn {
            return Err(Error::unsupported(
                "quiet nested raw equality outside interval functions",
            ));
        }
        let (lo, len) = (va.lo, va.data.len());
        if lo != vb.lo || len != vb.data.len() {
            return Ok(false);
        }
        for index in (0..len).rev() {
            let (x, y) = (self.view(a).data[index], self.view(b).data[index]);
            if !self.quiet_construction_equals(x, y)? {
                return Ok(false);
            }
        }
        Ok(true)
    }

    fn quiet_literal(&mut self, h: Handle) -> Result<Handle, Error> {
        if self.local_literal_index(h).is_none() {
            return Ok(h);
        }
        let values = self.history_copy(self.view(h).data)?;
        let copy = self.local_literal(&values)?;
        self.norm(copy)
    }

    fn snapshot_history(&mut self, index: usize) -> Result<Handle, Error> {
        self.history_poll()?;
        if self.arena.literal_sets[index].origin == 0 {
            self.arena.literal_sets[index].origin = self.store.history_origin()?;
        }
        if matches!(
            self.arena.literal_sets[index].phase,
            crate::arena::LiteralPhase::Normalized(_)
        ) {
            self.normalize_literal(index)?;
        }
        let entry = &self.arena.literal_sets[index];
        let normalized = !entry.normalized.is_empty();
        let slot = usize::from(normalized);
        if !entry.snapshots[slot].is_empty() {
            return Ok(entry.snapshots[slot]);
        }
        let h = if normalized {
            entry.normalized
        } else {
            entry.handle
        };
        let origin = entry.origin;
        let view = self.view(h);
        let (kind, flags, hash, sum) = (view.kind, view.flags, view.hash, view.sum);
        let mut data = self.history_copy(view.data)?;
        for child in &mut data {
            *child = self.intern_exact(*child)?;
        }
        let retained = self
            .store
            .intern(&mut self.cache, kind, flags, origin, hash, sum, &data);
        self.store.retain_history(origin, normalized, retained)?;
        self.arena.literal_sets[index].snapshots[slot] = retained;
        Ok(retained)
    }

    /// The string id of `text`.
    ///
    /// # Errors
    /// The string table is full.
    pub fn string(&self, text: &str) -> Result<Handle, Error> {
        Ok(Handle::string(self.store.strings().intern(text)?))
    }
}

/// The key of entry `i` of a function view.
pub(crate) fn fcn_key(v: &View<'_>, i: usize) -> Handle {
    match v.kind {
        #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
        Kind::IntFcn => Handle::int(v.lo + i as i64).unwrap_or(Handle::EMPTY),
        _ => v.data[i],
    }
}

/// The lower bound if `keys` are the consecutive integers `lo, lo+1, ...`.
fn contiguous_ints(keys: &[Handle]) -> Option<i64> {
    // The empty function is the empty tuple: one canonical form.
    let Some(first) = keys.first() else {
        return Some(1);
    };
    let lo = first.as_int()?;
    for (i, k) in keys.iter().enumerate() {
        #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
        if k.as_int()? != lo + i as i64 {
            return None;
        }
    }
    Some(lo)
}

/// The kind of value an odometer builds.
#[derive(Clone, Debug)]
pub enum Shape {
    /// A tuple of the components.
    Tuple,
    /// A function from these sorted keys to the components.
    Fcn(Vec<Handle>),
}

/// A cursor over a set's elements in canonical order.
#[derive(Clone, Debug)]
pub enum Cursor {
    /// An enumerated set.
    Array {
        /// The set.
        set: Handle,
        /// The next index.
        i: usize,
    },
    /// An interval.
    Range {
        /// The next element.
        cur: i64,
        /// The last element.
        hi: i64,
    },
    /// Functions, records or tuples over component sets, the last component
    /// fastest (canonical order).
    Odometer {
        /// What each step builds.
        shape: Shape,
        /// The component element lists.
        comps: Vec<Vec<Handle>>,
        /// The current indices.
        idx: Vec<usize>,
        /// Exhausted.
        done: bool,
    },
    /// `SUBSET S`: by size, then lexicographically.
    Subset {
        /// The base elements.
        elems: Vec<Handle>,
        /// The current size.
        k: usize,
        /// The current combination.
        combo: Vec<usize>,
        /// Whether the first combination of size `k` was produced.
        started: bool,
    },
}

impl Cursor {
    fn odometer(shape: Shape, comps: Vec<Vec<Handle>>) -> Self {
        let done = comps.iter().any(Vec::is_empty);
        let n = comps.len();
        Self::Odometer {
            shape,
            comps,
            idx: vec![0; n],
            done,
        }
    }

    /// The next element.
    ///
    /// # Errors
    /// A stale set handle.
    pub fn next(&mut self, heap: &mut Heap) -> Result<Option<Handle>, Error> {
        match self {
            Self::Array { set, i } => {
                let v = heap.view(*set);
                let r = v.data.get(*i).copied();
                *i += 1;
                Ok(r)
            }
            Self::Range { cur, hi } => {
                if *cur > *hi {
                    return Ok(None);
                }
                let r = Handle::int(*cur);
                *cur += 1;
                r.map(Some)
                    .ok_or_else(|| Error::unsupported("integer outside the 61-bit range"))
            }
            Self::Odometer {
                shape,
                comps,
                idx,
                done,
            } => {
                if *done {
                    return Ok(None);
                }
                let vals: Vec<Handle> = idx.iter().zip(comps.iter()).map(|(&i, c)| c[i]).collect();
                let out = match shape {
                    Shape::Tuple => heap.tuple(&vals),
                    Shape::Fcn(keys) => heap.fcn_sorted(keys, &vals),
                };
                // Advance, last position fastest.
                let mut p = idx.len();
                loop {
                    if p == 0 {
                        *done = true;
                        break;
                    }
                    p -= 1;
                    idx[p] += 1;
                    if idx[p] < comps[p].len() {
                        break;
                    }
                    idx[p] = 0;
                }
                Ok(Some(out))
            }
            Self::Subset {
                elems,
                k,
                combo,
                started,
            } => {
                let n = elems.len();
                if *started {
                    // Next combination of size k, lexicographically.
                    let mut i = *k;
                    let mut advanced = false;
                    while i > 0 {
                        i -= 1;
                        if combo[i] < n - *k + i {
                            combo[i] += 1;
                            for j in i + 1..*k {
                                combo[j] = combo[j - 1] + 1;
                            }
                            advanced = true;
                            break;
                        }
                    }
                    if !advanced {
                        *k += 1;
                        if *k > n {
                            return Ok(None);
                        }
                        *combo = (0..*k).collect();
                    }
                } else {
                    *started = true;
                    *combo = Vec::new();
                }
                let mut sub: Vec<Handle> = combo.iter().map(|&i| elems[i]).collect();
                Ok(Some(heap.set_from(&mut sub)))
            }
        }
    }
}

#[cfg(test)]
#[allow(clippy::many_single_char_names, reason = "short value names in tests")]
mod tests {
    use super::*;

    fn int(v: i64) -> Handle {
        Handle::int(v).unwrap_or(Handle::EMPTY)
    }

    fn heap(full: bool) -> Heap {
        Heap::new(Arc::new(Store::default()), false, full)
    }

    #[test]
    fn small_payload_updates_keep_layout_hashes_and_literal_classes() -> Result<(), Error> {
        for semantics in [crate::Semantics::default(), crate::Semantics::tlc()] {
            for boxed in [false, true] {
                for full in [false, true] {
                    let mut h = Heap::new(
                        Arc::new(Store::with_semantics(true, semantics)),
                        boxed,
                        full,
                    );
                    for n in [1, 4, 8, 9] {
                        let values: Vec<_> = (0..n)
                            .map(|i| int(i64::try_from(i).unwrap_or_default()))
                            .collect();
                        let keys: Vec<_> = (0..n)
                            .map(|i| h.string(&format!("field{i}")))
                            .collect::<Result<_, _>>()?;
                        for i in [0, n / 2, n - 1] {
                            let child = h.set_from(&mut vec![int(90), int(91)]);
                            for kind in 0..4 {
                                let source = match kind {
                                    0 => h.int_fcn(3, &values),
                                    1 => h.tuple_literal(&values),
                                    2 => h.fcn_sorted(&keys, &values),
                                    _ => h.record(&keys, &values),
                                };
                                let before = h.view(source);
                                let (node_kind, lo, flags) = (before.kind, before.lo, before.flags);
                                let mut expected = before.data.to_vec();
                                let slot = if node_kind == Kind::Fcn { n + i } else { i };
                                expected[slot] = child;
                                let updated = h.fcn_with(source, i, child);
                                let got = h.view(updated);
                                assert_eq!((got.kind, got.lo, got.flags), (node_kind, lo, flags));
                                assert_eq!(got.data, expected);
                                assert_eq!(h.hash(updated), h.deep_hash(updated));
                                assert_eq!(h.view(source).values(), values);
                                let retained = h.intern(updated)?;
                                assert_eq!(h.hash(retained), h.hash(updated));
                            }
                        }
                    }
                    h.reset();
                    assert!(!h.poisoned());
                }
            }
        }
        Ok(())
    }

    #[test]
    fn small_payload_updates_recount_laziness_and_retain_nested_history() -> Result<(), Error> {
        for boxed in [false, true] {
            for full in [false, true] {
                let mut h = Heap::new(Arc::new(Store::default()), boxed, full);
                let key = h.string("field")?;
                let domain = h.interval(1, 2);
                let lazy = h.lambda(7, domain, &[int(4)]);
                let record = h.record(&[key], &[lazy]);
                let closed = h.fcn_with(record, 0, int(8));
                assert!(!h.is_lazy(closed));
                assert!(h.view(closed).is_record_literal());
                let lazy_again = h.fcn_with(closed, 0, lazy);
                assert!(h.is_lazy(lazy_again));
                assert!(h.intern(lazy_again).is_err());
                let raw = h.local_literal(&[int(2), int(1), int(2)])?;
                let tuple = h.tuple_literal(&[int(0)]);
                let nested = h.fcn_with(tuple, 0, raw);
                assert!(h.view(nested).is_tuple_literal());
                assert!(h.has_construction_history(nested));
                let original = h.intern_exact(nested)?;
                let identity = h.intern(nested)?;
                let replaced = h.fcn_with(nested, 0, int(5));
                assert_ne!(h.view(replaced).flags & flags::NESTED_VIEW, 0);
                h.reset();
                assert!(h.has_construction_history(original));
                assert!(!h.has_construction_history(identity));
                assert!(!h.poisoned());
            }
        }
        Ok(())
    }

    #[test]
    fn small_payload_interning_closes_symbolic_and_view_children() -> Result<(), Error> {
        for semantics in [crate::Semantics::default(), crate::Semantics::tlc()] {
            for boxed in [false, true] {
                for hashcons in [false, true] {
                    let mut h = Heap::new(
                        Arc::new(Store::with_semantics(hashcons, semantics)),
                        boxed,
                        false,
                    );
                    let range = h.interval(1, 2);
                    let range = h.intern_const(range)?;
                    assert!(!h.intern_children_unchanged(&[range]));
                    let tuple = h.tuple_literal(&[int(3), int(4)]);
                    let view = h.intern_exact(tuple)?;
                    assert!(!h.intern_children_unchanged(&[view]));
                    let parent = h.tuple(&[range, view]);
                    let retained = h.intern(parent)?;
                    let data = h.view(retained).data.to_vec();
                    assert_eq!(h.view(data[0]).kind, Kind::Set);
                    assert_eq!(h.view(data[0]).data, &[int(1), int(2)]);
                    assert_eq!(h.view(data[1]).flags, 0);
                    assert!(h.intern_children_unchanged(&data));
                    for n in [0, 4, 8, 9] {
                        let values: Vec<_> = (0..n).map(|i| data[i % 2]).collect();
                        let root = h.int_fcn(0, &values);
                        let p = h.intern(root)?;
                        assert_eq!(h.intern(root)?, p);
                        assert_eq!(h.view(p).data, values);
                        assert_eq!(h.hash(p), h.hash(root));
                    }
                    let mark = h.mark();
                    let scratch = h.tuple(&[data[0], int(9)]);
                    let kept = h.intern(scratch)?;
                    h.rollback(mark);
                    assert_eq!(h.view(kept).data, &[data[0], int(9)]);
                    h.reset();
                    assert_eq!(h.view(retained).data, data);
                    assert!(!h.poisoned());
                }
            }
        }
        Ok(())
    }

    #[test]
    fn small_payload_intern_eligibility_preserves_poison_and_rejection_order() -> Result<(), Error>
    {
        for boxed in [false, true] {
            let mut h = Heap::new(Arc::new(Store::default()), boxed, false);
            let invalid = Handle::persistent(0, 0, crate::handle::MAX_CLASS_INDEX);
            let transient = h.tuple(&[int(1)]);
            assert!(!h.intern_children_unchanged(&[transient, invalid]));
            assert!(
                !h.poisoned(),
                "a rejected prefix must not read later children"
            );
            let range = h.interval(1, 2);
            let range = h.intern_const(range)?;
            assert!(!h.intern_children_unchanged(&[range, invalid]));
            assert!(!h.poisoned());
            let lazy = h.lambda(1, range, &[]);
            let lazy = h.intern_const(lazy)?;
            assert!(!h.intern_children_unchanged(&[lazy, invalid]));
            assert!(!h.poisoned());
            // Deliberately bypass builder flag propagation to exercise child
            // error ordering independently of the parent's lazy rejection.
            let parent = h.arena.alloc(Kind::IntFcn, 0, 1, 0, 0, &[lazy, invalid]);
            assert!(h.intern(parent).is_err());
            assert!(!h.poisoned(), "later invalid child stays unread on error");
            let huge = h.interval(0, i64::try_from(MAX_MATERIALIZE).unwrap_or_default());
            let huge = h.intern_const(huge)?;
            let parent = h.arena.alloc(Kind::IntFcn, 0, 1, 0, 0, &[huge, invalid]);
            assert!(h.intern(parent).is_err());
            assert!(!h.poisoned(), "materialization error precedes later poison");
            assert!(h.intern_children_unchanged(&[invalid]));
            assert!(h.poisoned(), "the original persistent view read must occur");
        }
        Ok(())
    }

    #[test]
    fn small_payload_interning_keeps_store_and_arena_cache_accounting() -> Result<(), Error> {
        for boxed in [false, true] {
            for hashcons in [false, true] {
                let mut h = Heap::new(
                    Arc::new(Store::with_semantics(hashcons, crate::Semantics::default())),
                    boxed,
                    false,
                );
                let root = h.tuple(&[int(1), int(2)]);
                let retained = h.intern(root)?;
                h.store.flush_hits(&mut h.cache);
                let before = h.store.stats();
                assert_eq!(h.intern(root)?, retained);
                h.store.flush_hits(&mut h.cache);
                assert_eq!(
                    h.store.stats(),
                    before,
                    "arena hit does not probe the store"
                );
                let equal = h.tuple(&[int(1), int(2)]);
                let other = h.intern(equal)?;
                h.store.flush_hits(&mut h.cache);
                let after = h.store.stats();
                if hashcons {
                    assert_eq!(other, retained);
                    assert_eq!(after.hits, before.hits + 1);
                    assert_eq!(after.nodes, before.nodes);
                } else {
                    assert_ne!(other, retained);
                    assert_eq!(after.misses, before.misses + 1);
                    assert_eq!(after.nodes, before.nodes + 1);
                }
                let mark = h.mark();
                let stale = h.tuple(&[int(9)]);
                h.rollback(mark);
                assert_eq!(h.intern(stale)?, Handle::EMPTY);
                assert!(h.poisoned());
            }
        }
        Ok(())
    }

    #[test]
    fn sorted_construction_miss_matches_every_original_class_scan() {
        for semantics in [crate::Semantics::default(), crate::Semantics::tlc()] {
            let mut h = Heap::new(
                Arc::new(Store::with_semantics(true, semantics)),
                false,
                false,
            );
            let names: Vec<_> = ["u", "A_x", "B_x", "z", "A_y"]
                .into_iter()
                .map(|s| Handle::model(h.store().strings().intern(s).unwrap_or(u32::MAX)))
                .collect();
            let text = h.string("text").unwrap_or(Handle::EMPTY);
            let big = h.int_from_i64(i64::MAX);
            let tuple = h.tuple_literal(&[int(1), names[0]]);
            let record = h.record(&[text], &[int(1)]);
            let set = h.set_from(&mut vec![int(1), int(2)]);
            let empty = h.set_from(&mut Vec::new());
            let values = [
                names[0],
                names[1],
                names[2],
                Handle::FALSE,
                Handle::TRUE,
                int(1),
                big,
                text,
                empty,
                set,
                tuple,
                record,
            ];
            let nested = h.tuple_literal(&[tuple]);
            let queries = [
                names[0],
                names[1],
                names[2],
                names[3],
                names[4],
                Handle::FALSE,
                Handle::TRUE,
                int(2),
                big,
                text,
                empty,
                set,
                tuple,
                record,
                nested,
            ];
            for mask in 0..(1 << values.len()) {
                let mut sorted: Vec<_> = values
                    .iter()
                    .enumerate()
                    .filter_map(|(i, &v)| (mask & (1 << i) != 0).then_some(v))
                    .collect();
                sorted.sort_unstable_by(|&a, &b| h.cmp(a, b));
                for query in queries {
                    let original = sorted.iter().any(|&value| {
                        crate::compare::default_member_needs_checked(&h, query, value)
                    });
                    assert_eq!(
                        h.construction_sorted_needs_checked(query, &sorted),
                        Ok(original),
                        "query {query:?}, values {sorted:?}"
                    );
                }
            }
            assert!(!h.poisoned());
        }
    }

    #[test]
    fn sorted_construction_model_prefix_polls_the_existing_owner() {
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
        #[derive(Debug)]
        struct Stop(AtomicUsize);
        impl crate::eval::WorkControl for Stop {
            fn proceed(&self) -> bool {
                self.0.fetch_add(1, AtomicOrdering::Relaxed) < 2
            }
        }
        let mut h = heap(false);
        let mut values: Vec<_> = (0..4096)
            .map(|i| {
                Handle::model(
                    h.store()
                        .strings()
                        .intern(&format!("u{i}"))
                        .unwrap_or(u32::MAX),
                )
            })
            .collect();
        values.sort_unstable_by(|&a, &b| h.cmp(a, b));
        h.history_steps.set(0);
        h.set_history_control(Some(Arc::new(Stop(AtomicUsize::new(0)))));
        assert!(matches!(
            h.construction_sorted_needs_checked(int(1), &values),
            Err(error) if error.kind() == crate::ErrorKind::Stop
        ));
        assert!(h.work_interrupted());
    }

    /// The heap's copy of the string texts orders strings as the table
    /// does, also for strings interned after the copy was first made.
    #[test]
    fn string_comparison_follows_the_table_as_it_grows() {
        let h = heap(false);
        let t = h.store().strings();
        let id = |s: &str| t.intern(s).unwrap_or(u32::MAX);
        let (b, a) = (id("b"), id("a"));
        assert_eq!(h.str_cmp(b, a), Ordering::Greater);
        let (c, aa) = (id("c"), id("aa"));
        for (x, y) in [(a, b), (b, c), (aa, a), (aa, b), (c, aa), (c, c)] {
            assert_eq!(h.str_cmp(x, y), t.cmp(x, y), "{x} {y}");
        }
    }

    #[test]
    fn literal_alias_history_survives_rollback_without_a_postmark_handle() {
        let store = Arc::new(Store::with_semantics(false, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let one = Handle::int(1).unwrap_or(Handle::EMPTY);
        let two = Handle::int(2).unwrap_or(Handle::EMPTY);
        let three = Handle::int(3).unwrap_or(Handle::EMPTY);
        let raw = [three, one, two, two];
        let set = h.local_literal(&raw).unwrap_or(Handle::EMPTY);
        let alias = set;
        let independent = h.local_literal(&raw).unwrap_or(Handle::EMPTY);
        assert_ne!(
            set, independent,
            "equal constructions do not share mutable history"
        );
        assert_eq!(h.literal_member(two, set), Some(Ok(true)));
        assert_eq!(
            h.arena.literal_trace,
            vec![
                ("equals", three, two),
                ("equals", one, two),
                ("equals", two, two)
            ]
        );
        h.arena.literal_trace.clear();
        let mark = h.mark();
        let postmark = h.local_literal(&raw).unwrap_or(Handle::EMPTY);
        assert!(h.norm(alias).is_ok());
        assert_eq!(
            h.arena.literal_trace,
            vec![
                ("insert", one, three),
                ("insert", two, three),
                ("insert", two, one),
                ("insert", two, two)
            ]
        );
        h.rollback(mark);
        let restored = h.norm(alias).unwrap_or(Handle::EMPTY);
        assert_eq!(h.view(restored).data, &[one, two, three]);
        assert!(
            h.intern_exact(alias).is_ok(),
            "normalization ownership survives rolled-back projection storage"
        );
        assert_eq!(h.local_literal_index(postmark), None);
        assert_eq!(h.arena.literal_sets.len(), 2);
        h.arena.literal_trace.clear();
        assert_eq!(h.literal_member(two, set), Some(Ok(true)));
        assert_eq!(h.arena.literal_trace, vec![("search", two, two)]);
        h.arena.literal_trace.clear();
        assert_eq!(h.literal_member(one, set), Some(Ok(true)));
        assert_eq!(
            h.arena.literal_trace,
            vec![("search", one, two), ("search", one, one)]
        );
        h.arena.literal_trace.clear();
        assert_eq!(h.literal_member(two, independent), Some(Ok(true)));
        assert_eq!(
            h.arena.literal_trace,
            vec![
                ("equals", three, two),
                ("equals", one, two),
                ("equals", two, two)
            ]
        );
        h.reset();
        assert!(h.arena.literal_sets.is_empty());
        let replacement = h.local_literal(&raw).unwrap_or(Handle::EMPTY);
        assert_ne!(replacement, set);
        h.arena.literal_trace.clear();
        assert_eq!(h.literal_member(two, replacement), Some(Ok(true)));
        assert_eq!(h.arena.literal_trace.len(), 3);
        assert!(!h.poisoned());
    }

    #[test]
    fn literal_eligibility_bounds_constructor_query_and_owned_array() {
        let store = Arc::new(Store::with_semantics(false, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let values: Vec<_> = (0..63)
            .map(|i| Handle::int(i).unwrap_or(Handle::EMPTY))
            .collect();
        let set = h.local_literal(&values).unwrap_or(Handle::EMPTY);
        assert!(h.norm(set).is_ok());
        let phase = &h.arena.literal_sets[0].phase;
        assert!(
            matches!(phase, crate::arena::LiteralPhase::Normalized(_)),
            "the bounded normalization must execute"
        );
        if let crate::arena::LiteralPhase::Normalized(array) = phase {
            assert_eq!(array.len() * std::mem::size_of::<Handle>(), 504);
        }
        let one = Handle::int(1).unwrap_or(Handle::EMPTY);
        let repeated = h.local_literal(&vec![one; 1024]).unwrap_or(Handle::EMPTY);
        let repeated = h.norm(repeated).unwrap_or(Handle::EMPTY);
        assert_eq!(
            h.view(repeated).data,
            &[one],
            "collection length is governed by actual resources; per-value eligibility remains bounded"
        );
        let large_query = h.tuple_literal(&vec![one; 64]);
        assert!(matches!(h.literal_member(large_query, set), Some(Err(_))));
        assert!(!h.poisoned());
    }

    #[test]
    fn retained_origins_aliases_and_nested_updates_survive_reset_without_forging_authority() {
        let mut h = heap(false);
        let one = int(1);
        let two = int(2);
        let raw = h.local_literal(&[two, one, two]).unwrap_or(Handle::EMPTY);
        let independent = h.local_literal(&[two, one, two]).unwrap_or(Handle::EMPTY);
        let alias = raw;
        let norm = h.norm(raw).unwrap_or(Handle::EMPTY);
        assert_eq!(h.literal_member(one, alias), Some(Ok(true)));
        assert_eq!(h.literal_member(one, independent), Some(Ok(true)));
        let retained = h.intern_exact(norm).unwrap_or(Handle::EMPTY);
        let same = h.intern_exact(alias).unwrap_or(Handle::EMPTY);
        assert_eq!(
            same, retained,
            "actual aliases retain one origin even with hashcons off"
        );
        let independent = h.norm(independent).unwrap_or(Handle::EMPTY);
        let other = h.intern_exact(independent).unwrap_or(Handle::EMPTY);
        assert_ne!(
            retained, other,
            "equal independent constructions do not acquire alias identity"
        );
        let before = h.tuple_literal(&[raw]);
        let nested = h.fcn_with(before, 0, norm);
        assert!(h.has_construction_history(nested));
        let nested = h.normalize_construction(nested).unwrap_or(Handle::EMPTY);
        let parent = h.intern_exact(nested).unwrap_or(Handle::EMPTY);
        let canonical = h.intern(parent).unwrap_or(Handle::EMPTY);
        assert!(h.eq(parent, canonical));
        let view = h.view(retained);
        let forged = h.store.intern(
            &mut crate::store::InternCache::default(),
            Kind::Set,
            flags::HISTORY,
            view.lo + 1000,
            view.hash,
            view.sum,
            view.data,
        );
        h.reset();
        assert_eq!(h.literal_member(one, retained), Some(Ok(true)));
        assert_eq!(
            h.literal_member(one, forged),
            None,
            "flags/Kind/is_norm alone cannot certify a constructor"
        );
        assert!(h.normalize_construction(forged).is_err());
        let a = h.intern(retained).unwrap_or(Handle::EMPTY);
        let b = h.intern(other).unwrap_or(Handle::EMPTY);
        assert!(h.eq(a, b));
        assert!(h.store.memory_usage().history_view_bytes > 0);
        assert!(!h.poisoned());
    }

    #[test]
    fn construction_projection_copy_polls_the_installed_owner_mid_copy() {
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
        #[derive(Debug)]
        struct Stop(AtomicUsize);
        impl crate::eval::WorkControl for Stop {
            fn proceed(&self) -> bool {
                self.0.fetch_add(1, AtomicOrdering::Relaxed) < 2
            }
        }
        let mut h = heap(false);
        let values = vec![int(1); 16384];
        let owner = Arc::new(Stop(AtomicUsize::new(0)));
        h.history_steps.set(0);
        h.set_history_control(Some(owner.clone()));
        assert!(
            matches!(h.history_copy(&values), Err(error) if error.kind() == crate::ErrorKind::Stop)
        );
        assert_eq!(owner.0.load(AtomicOrdering::Relaxed), 3);
        assert!(h.work_interrupted());
        assert!(
            h.arena.literal_sets.is_empty(),
            "an interrupted copy creates no constructor"
        );
    }

    #[test]
    fn normalization_and_raw_scan_poll_the_installed_owner() {
        use std::sync::atomic::{AtomicUsize, Ordering as AtomicOrdering};
        #[derive(Debug)]
        struct Stop(AtomicUsize);
        impl crate::eval::WorkControl for Stop {
            fn proceed(&self) -> bool {
                self.0.fetch_add(1, AtomicOrdering::Relaxed) < 2
            }
        }
        let mut h = heap(false);
        let values: Vec<_> = (0..4096).rev().map(int).collect();
        let set = h.local_literal(&values).unwrap_or(Handle::EMPTY);
        h.set_history_control(Some(Arc::new(Stop(AtomicUsize::new(0)))));
        assert!(matches!(h.norm(set), Err(error) if error.kind() == crate::ErrorKind::Stop));
        assert!(h.work_interrupted());
        assert!(
            matches!(
                h.arena.literal_sets[0].phase,
                crate::arena::LiteralPhase::Raw
            ),
            "interruption is not successful normalization"
        );
        h.reset();
        h.set_history_control(None);
        let set = h.local_literal(&values).unwrap_or(Handle::EMPTY);
        h.set_history_control(Some(Arc::new(Stop(AtomicUsize::new(0)))));
        assert!(
            matches!(h.construction_member(int(-1), set, true, None), Some(Err(error)) if error.kind() == crate::ErrorKind::Stop)
        );
    }

    #[test]
    fn model_and_string_ranks_use_real_tokens_for_insertion_and_profile_order_afterward() {
        for semantics in [crate::Semantics::default(), crate::Semantics::tlc()] {
            let store = Arc::new(Store::with_semantics(true, semantics));
            let z = store.strings().intern("z").unwrap_or(u32::MAX);
            let a = store.strings().intern("a").unwrap_or(u32::MAX);
            for (first, second) in [
                (Handle::model(z), Handle::model(a)),
                (Handle::string(z), Handle::string(a)),
            ] {
                let mut h = Heap::new(Arc::clone(&store), false, false);
                let raw = h.local_literal(&[second, first]).unwrap_or(Handle::EMPTY);
                let norm = h.norm(raw).unwrap_or(Handle::EMPTY);
                assert_eq!(h.arena.literal_trace, vec![("insert", first, second)]);
                let expected = if semantics.tlc_comparisons {
                    [first, second]
                } else {
                    [second, first]
                };
                assert_eq!(h.view(norm).data, expected);
                assert_eq!(h.literal_member(first, raw), Some(Ok(true)));
                let retained = h.intern_exact(norm).unwrap_or(Handle::EMPTY);
                h.reset();
                assert_eq!(h.literal_member(second, retained), Some(Ok(true)));
            }
        }
    }

    #[test]
    fn literal_even_search_uses_query_receiver_and_valuevec_pivots() {
        let store = Arc::new(Store::with_semantics(false, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let one = Handle::int(1).unwrap_or(Handle::EMPTY);
        let two = Handle::int(2).unwrap_or(Handle::EMPTY);
        let three = Handle::int(3).unwrap_or(Handle::EMPTY);
        let four = Handle::int(4).unwrap_or(Handle::EMPTY);
        let set = h
            .local_literal(&[four, one, three, two])
            .unwrap_or(Handle::EMPTY);
        assert!(h.norm(set).is_ok());
        h.arena.literal_trace.clear();
        assert_eq!(h.literal_member(two, set), Some(Ok(true)));
        assert_eq!(
            h.arena.literal_trace,
            vec![("search", two, three), ("search", two, two)]
        );
    }

    #[test]
    fn literal_normalization_retains_first_equal_tuple_receiver_and_unknowns() {
        let store = Arc::new(Store::with_semantics(false, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let one = Handle::int(1).unwrap_or(Handle::EMPTY);
        let first = h.tuple_literal(&[one]);
        let duplicate = h.tuple_literal(&[one]);
        assert_ne!(first, duplicate);
        let set = h
            .local_literal(&[first, duplicate])
            .unwrap_or(Handle::EMPTY);
        let raw_snapshot = h.intern_exact(set).unwrap_or(Handle::EMPTY);
        assert_eq!(h.view(raw_snapshot).kind, Kind::SetEnum);
        let normalized = h.norm(set).unwrap_or(Handle::EMPTY);
        assert_eq!(h.view(normalized).data, &[first]);
        let identity = h.intern(set).unwrap_or(Handle::EMPTY);
        let snapshot = h.intern_exact(set).unwrap_or(Handle::EMPTY);
        assert_ne!(
            snapshot, identity,
            "construction view remains apart from extensional identity"
        );
        assert_eq!(h.view(snapshot).data.len(), 1);
        assert!(h.store.owns_history(snapshot));
        assert!(
            h.intern(normalized).is_ok(),
            "the demanded extensional identity can be stored"
        );
        let general = h.tuple(&[one]);
        assert!(
            h.local_literal(&[general]).is_err(),
            "a function layout is not a tuple receiver proof"
        );
        let name = h.string("name").unwrap_or(Handle::EMPTY);
        assert!(h.local_literal(&[name]).is_ok());
        let set = h.local_literal(&[one]).unwrap_or(Handle::EMPTY);
        assert!(matches!(
            h.literal_member(Handle::model(u32::MAX), set),
            Some(Err(_))
        ));
        assert!(matches!(
            h.literal_member(Handle::string(u32::MAX), set),
            Some(Err(_))
        ));
        let unknown = h.set_enum(&[one]);
        assert_eq!(
            h.literal_member(one, unknown),
            None,
            "raw kind does not certify construction history"
        );
        let slot = h.view(set).lo;
        let forged = h.symbolic(Kind::SetEnum, slot, &[one]);
        assert_eq!(
            h.literal_member(one, forged),
            None,
            "a slot alone is not an owned history identity"
        );
        assert!(!h.poisoned());
    }

    #[test]
    fn model_types_follow_utf16_names_across_append_and_reset() {
        use crate::strings::StringOrder;

        for order in [StringOrder::Text, StringOrder::Token] {
            let mut h = Heap::new(
                Arc::new(Store::with_string_order(true, order)),
                false,
                false,
            );
            for names in [
                ["", "a", "A_", "A_x", "_x", "n1", "é_x", "😀_x"],
                ["é_😀", "𐀀_x", "A_é", "A_😀", "__x", "_", "😀", "x_y"],
            ] {
                for name in names {
                    let id = h.store.strings().intern(name).unwrap_or(u32::MAX);
                    let units: Vec<_> = name.encode_utf16().collect();
                    let expected = if units.len() > 2 && units[1] == u16::from(b'_') {
                        Some(units[0])
                    } else {
                        None
                    };
                    for _ in 0..2 {
                        assert_eq!(h.model_type(id), expected, "{order:?} {name:?}");
                        assert_eq!(crate::compare::mv_type(&h, Handle::model(id)), expected);
                        h.reset();
                    }
                }
            }
            assert_eq!(h.model_type(u32::MAX), None);
            assert_eq!(crate::compare::mv_type(&h, Handle::TRUE), None);
            h.store.strings().seal();
            assert_eq!(h.model_type(3), Some(u16::from(b'A')));
        }
    }

    #[test]
    fn incremental_except_hash_equals_the_full_hash() {
        for full in [false, true] {
            let mut h = heap(full);
            let f = h.int_fcn(1, &[int(1), int(2), int(3)]);
            let inner = h.set_from(&mut vec![int(7), int(5)]);
            let g = h.fcn_with(f, 1, inner);
            let direct = h.int_fcn(1, &[int(1), inner, int(3)]);
            assert!(h.eq(g, direct));
            assert_eq!(h.hash(g), h.hash(direct));
            assert_eq!(h.hash(g), h.deep_hash(direct));
            // Records: string keys, through `fcn_with` twice.
            let (a, b) = (
                h.string("a").unwrap_or(Handle::EMPTY),
                h.string("b").unwrap_or(Handle::EMPTY),
            );
            let r = h.fcn_from_pairs(&mut [(b, int(1)), (a, int(2))]);
            let r2 = h.fcn_with(r, 0, int(9));
            let r3 = h.fcn_with(r2, 0, int(2));
            assert_eq!(h.hash(r3), h.deep_hash(r));
            assert!(h.eq(r3, r));
        }
    }

    #[test]
    fn interning_is_hash_consed_and_preserves_hashes() {
        let mut h = heap(false);
        let a = h.set_from(&mut vec![int(3), int(1), int(2)]);
        let b = h.interval(1, 3);
        let b = h.norm(b).unwrap_or(Handle::EMPTY);
        assert!(h.eq(a, b));
        let pa = h.intern(a).unwrap_or(Handle::EMPTY);
        let pb = h.intern(b).unwrap_or(Handle::EMPTY);
        assert_eq!(pa, pb);
        assert_eq!(h.hash(pa), h.hash(a));
        let t = h.tuple(&[a, int(4)]);
        let pt = h.intern(t).unwrap_or(Handle::EMPTY);
        h.reset();
        // After the epoch ends the persistent copy still resolves.
        let v = h.view(pt);
        assert_eq!(v.data.first().copied(), Some(pa));
        assert!(!h.poisoned());
        let st = h.store().stats();
        assert!(st.hits >= 1, "{st:?}");
    }

    #[test]
    fn stale_transient_handles_poison_the_heap() {
        let mut h = heap(false);
        let a = h.set_from(&mut vec![int(1)]);
        h.reset();
        let _ = h.view(a);
        assert!(h.poisoned());
    }

    #[test]
    fn rollback_frees_candidates_and_stale_reads_poison() {
        let mut h = heap(false);
        let kept = h.set_from(&mut vec![int(1)]);
        for i in 0..1000 {
            let m = h.mark();
            let c = h.tuple(&[int(i), kept]);
            if i % 10 == 0 {
                continue;
            }
            assert!(h.eq(h.view(c).data[1], kept));
            h.rollback(m);
        }
        // The 100 kept candidates and the set remain; the rest were freed.
        assert_eq!(h.arena().len(), 101);
        assert!(!h.poisoned());
        let m = h.mark();
        let gone = h.tuple(&[int(7)]);
        h.rollback(m);
        let _ = h.tuple(&[int(8)]);
        let _ = h.view(gone);
        assert!(h.poisoned(), "a handle freed by rollback must not resolve");
    }

    #[test]
    fn lazy_values_carry_their_closure_and_propagate() {
        let mut h = heap(false);
        let dom = h.interval(1, 3);
        let f = h.lambda(7, dom, &[int(5)]);
        assert!(h.is_lazy(f));
        let c = h
            .closure(f)
            .map(|c| (c.kind, c.id, c.domain, c.captures.to_vec()));
        assert_eq!(c, Some((Kind::Lambda, 7, dom, vec![int(5)])));
        // A record holding it is lazy; a closed one is not.
        let a = h.string("a").unwrap_or(Handle::EMPTY);
        let r = h.fcn_sorted(&[a], &[f]);
        assert!(h.is_lazy(r));
        let closed = h.fcn_sorted(&[a], &[int(1)]);
        assert!(!h.is_lazy(closed));
        assert!(!h.poisoned());
        // Interning an unclosed value is refused.
        assert!(h.intern(r).is_err());
        assert!(!h.poisoned());
        // EXCEPT replacing the only lazy entry recounts laziness.
        let r2 = h.fcn_with(r, 0, int(1));
        assert!(!h.is_lazy(r2));
        assert!(h.eq(r2, closed));
        let p = h.set_pred(2, dom, &[]);
        assert!(h.is_lazy(p));
        assert_eq!(h.norm(p).ok(), Some(p), "only the evaluator enumerates it");
        assert!(h.cursor(p).is_err());
        assert!(!h.poisoned());
    }

    #[test]
    fn lazy_values_in_closed_positions_poison() {
        for op in 0..3 {
            let mut h = heap(false);
            let dom = h.interval(1, 3);
            let f = h.lambda(0, dom, &[]);
            match op {
                0 => {
                    let _ = h.hash(f);
                }
                1 => {
                    let _ = h.set_sorted(&[f]);
                }
                _ => {
                    let g = h.lambda(1, dom, &[]);
                    let _ = h.eq(f, g);
                }
            }
            assert!(h.poisoned(), "op {op}");
        }
    }

    #[test]
    fn rebuild_closes_a_value_bottom_up() {
        let mut h = heap(false);
        let dom = h.interval(1, 2);
        let f = h.lambda(0, dom, &[]);
        let t = h.tuple(&[int(9), f]);
        // The evaluator materializes `f` and rebuilds the parent.
        let m = h.int_fcn(1, &[int(10), int(20)]);
        let data = h.view(t).data.to_vec();
        let closed = h.rebuild(t, &[data[0], m]);
        assert!(!h.is_lazy(closed));
        let direct = h.tuple(&[int(9), m]);
        assert!(h.eq(closed, direct));
        assert_eq!(h.hash(closed), h.hash(direct));
        assert!(h.intern(closed).is_ok());
        // A rebuilt set is re-sorted.
        let s = h.set_from(&mut vec![int(1), int(2)]);
        let s2 = h.rebuild(s, &[int(5), int(3)]);
        assert_eq!(h.view(s2).data, &[int(3), int(5)]);
        assert!(!h.poisoned());
    }

    #[test]
    fn lazy_constants_stay_lazy_and_are_never_compared_by_handle() {
        let mut h = heap(false);
        let nat = h.symbolic(Kind::Nat, 0, &[]);
        let f = h.lambda(3, nat, &[]);
        let p = h.intern_const(f).unwrap_or(Handle::EMPTY);
        assert!(p.is_lazy_persistent());
        h.reset();
        assert_eq!(h.closure(p).map(|c| c.id), Some(3));
        assert!(h.intern(p).is_err());
        let nat = h.symbolic(Kind::Nat, 0, &[]);
        let g = h.lambda(4, nat, &[]);
        let q = h.intern_const(g).unwrap_or(Handle::EMPTY);
        assert!(!h.poisoned());
        let _ = h.eq(p, q);
        assert!(h.poisoned(), "two lazy constants cannot be compared");
    }

    #[test]
    fn nested_function_views_never_claim_canonical_identity() -> Result<(), Error> {
        for hashcons in [true, false] {
            for exact in [false, true] {
                let store = Arc::new(Store::new(hashcons));
                let mut h = Heap::new(store, false, false);
                let a = h.string("a")?;
                let record = h.record(&[a], &[int(0)]);
                let general = h.fcn_sorted(&[a], &[int(0)]);
                let left = h.fcn_sorted(&[int(1)], &[record]);
                let right = h.fcn_sorted(&[int(1)], &[general]);
                let left = h.fcn_sorted(&[int(1)], &[left]);
                let right = h.fcn_sorted(&[int(1)], &[right]);
                let (left, right) = if exact {
                    (h.intern_exact(left)?, h.intern_exact(right)?)
                } else {
                    (h.intern_const(left)?, h.intern_const(right)?)
                };
                h.reset();
                assert!(
                    h.eq(left, right),
                    "equal values cannot be rejected by the persistent identity fast path"
                );
                assert!(
                    left.is_lazy_persistent(),
                    "nested representation is structural"
                );
                assert_eq!(
                    h.view(left).flags & flags::TUPLE,
                    0,
                    "the parent is still a general function"
                );
                assert_ne!(left, right, "representations remain distinct");
                assert_eq!(h.hash(left), h.hash(right));
                let a = h.intern(left)?;
                let b = h.intern(right)?;
                assert!(!a.is_lazy_persistent());
                assert!(h.eq(a, b));
                if hashcons {
                    assert_eq!(a, b, "canonical identity is unchanged");
                }
            }
        }
        Ok(())
    }

    #[test]
    fn unions_stay_symbolic_until_normalized() {
        let mut h = heap(false);
        let a = h.interval(1, 3);
        let b = h.set_from(&mut vec![int(0), int(2), int(7)]);
        let u = h.union(a, b);
        assert_eq!(h.kind(u), Some(Kind::Union));
        assert!(h.is_finite(u));
        assert_eq!(h.cardinality(u).ok(), Some(5));
        let n = h.norm(u).unwrap_or(Handle::EMPTY);
        assert_eq!(h.view(n).data, &[int(0), int(1), int(2), int(3), int(7)]);
        let nat = h.symbolic(Kind::Nat, 0, &[]);
        let inf = h.union(nat, b);
        assert!(!h.is_finite(inf));
        assert_eq!(h.norm(inf).ok(), Some(inf));
        assert!(!h.poisoned());
    }

    #[test]
    fn intern_view_keeps_the_representation_beside_the_identity() {
        let mut h = heap(false);
        let iv = h.interval(1, 3);
        let (id, view) = h.intern_view(iv).unwrap_or((Handle::EMPTY, Handle::EMPTY));
        assert_eq!(h.kind(id), Some(Kind::Set));
        assert_eq!(h.kind(view), Some(Kind::Interval));
        let e = h.set_from(&mut vec![int(3), int(2), int(1)]);
        let (id2, none) = h.intern_view(e).unwrap_or((Handle::EMPTY, Handle::TRUE));
        assert_eq!(id, id2, "identity is extensional");
        assert!(none.is_empty());
        assert!(!h.poisoned());
    }

    #[test]
    fn sets_of_strings_follow_the_store_order() {
        use crate::strings::StringOrder;
        for (order, want) in [
            (StringOrder::Text, ["a", "b"]),
            (StringOrder::Token, ["b", "a"]),
        ] {
            let store = Arc::new(Store::with_string_order(true, order));
            assert!(store.strings().preload(["b", "a"]).is_ok());
            let mut h = Heap::new(store, false, false);
            let (a, b) = (
                h.string("a").unwrap_or(Handle::EMPTY),
                h.string("b").unwrap_or(Handle::EMPTY),
            );
            let set = h.set_from(&mut vec![a, b]);
            let got: Vec<String> = h
                .view(set)
                .data
                .iter()
                .filter_map(|e| e.as_str_id())
                .filter_map(|id| h.store().strings().text(id))
                .map(|t| t.to_string())
                .collect();
            assert_eq!(got, want, "{order:?}");
        }
    }

    #[test]
    fn an_exact_copy_keeps_every_view_bit_and_symbolic_set() {
        let mut h = heap(false);
        let (a, b) = (
            h.string("a").unwrap_or(Handle::EMPTY),
            h.string("b").unwrap_or(Handle::EMPTY),
        );
        let span = h.interval(1, 3);
        let inner = h.record(&[a], &[int(1)]);
        let lit = h.record(&[a, b], &[inner, span]);
        let exact = h.intern_exact(lit).unwrap_or(Handle::EMPTY);
        assert!(exact.is_persistent());
        let v = h.view(exact);
        assert!(v.is_record_literal());
        let (first, second) = (v.values()[0], v.values()[1]);
        assert!(h.view(first).is_record_literal(), "nested bits are kept");
        assert_eq!(h.view(second).kind, Kind::Interval, "symbolic sets stay");
        assert!(h.eq(exact, lit));
        // Equal representations share one handle; the identity is apart.
        let inner2 = h.record(&[a], &[int(1)]);
        let span2 = h.interval(1, 3);
        let lit2 = h.record(&[a, b], &[inner2, span2]);
        assert_eq!(h.intern_exact(lit2).ok(), Some(exact));
        let fcn = h.fcn_sorted(&[a, b], &[inner2, span2]);
        assert_ne!(h.intern_exact(fcn).ok(), Some(exact));
    }

    #[test]
    fn a_record_literal_bit_is_a_view_bit() {
        let mut h = heap(false);
        let (a, b) = (
            h.string("a").unwrap_or(Handle::EMPTY),
            h.string("b").unwrap_or(Handle::EMPTY),
        );
        let (keys, vals) = ([a, b], [int(1), int(2)]);
        let lit = h.record(&keys, &vals);
        let fcn = h.fcn_sorted(&keys, &vals);
        assert!(h.view(lit).is_record_literal());
        assert!(!h.view(fcn).is_record_literal());
        assert!(h.eq(lit, fcn), "equality ignores the bit");
        assert_eq!(h.hash(lit), h.hash(fcn));
        assert_eq!(h.cmp(lit, fcn), Ordering::Equal);
        let changed = h.fcn_with(lit, 0, int(9));
        assert!(h.view(changed).is_record_literal(), "EXCEPT keeps it");

        // The identity never carries the bit, whichever is interned first.
        let id_lit = h.intern(lit).unwrap_or(Handle::EMPTY);
        let id_fcn = h.intern(fcn).unwrap_or(Handle::TRUE);
        assert_eq!(id_lit, id_fcn);
        assert!(!h.view(id_lit).is_record_literal());
        let lit2 = h.record(&keys, &vals);
        let (id, view) = h
            .intern_view(lit2)
            .unwrap_or((Handle::EMPTY, Handle::EMPTY));
        assert_eq!(id, id_fcn);
        assert!(h.view(view).is_record_literal());
        assert_ne!(view, id, "the view is interned apart from the identity");
        assert!(h.eq(view, id));
        assert_eq!(h.intern(view).ok(), Some(id));
        let again = h.record(&keys, &vals);
        let (_, view2) = h
            .intern_view(again)
            .unwrap_or((Handle::EMPTY, Handle::EMPTY));
        assert_eq!(view2, view, "views are hash-consed among themselves");
        let k = h.record(&keys, &vals);
        let constant = h.intern_const(k).unwrap_or(Handle::EMPTY);
        assert!(h.view(constant).is_record_literal());
        assert_eq!(
            h.intern(constant).ok(),
            Some(id),
            "identity remains canonical"
        );
        // A set holding the view and the identity has one element.
        let set = h.set_from(&mut vec![view, id]);
        assert_eq!(h.view(set).data.len(), 1);
        // Tuple literals likewise.
        let t = h.tuple_literal(&[int(1), int(2)]);
        let plain = h.tuple(&[int(1), int(2)]);
        assert!(h.view(t).is_tuple_literal() && !h.view(plain).is_tuple_literal());
        assert!(h.eq(t, plain));
        let t2 = h.fcn_with(t, 1, int(5));
        assert!(h.view(t2).is_tuple_literal());
        let (tid, tview) = h.intern_view(t).unwrap_or((Handle::EMPTY, Handle::EMPTY));
        assert_eq!(h.intern(plain).ok(), Some(tid));
        assert!(h.view(tview).is_tuple_literal() && !h.view(tid).is_tuple_literal());
        assert!(!h.poisoned());
    }

    #[test]
    fn a_set_enumeration_keeps_tlc_construction_order_as_a_view() {
        let mut h = heap(false);
        let e = h.set_enum(&[int(3), int(1), int(3), int(2)]);
        let v = h.view(e);
        assert_eq!(v.kind, Kind::SetEnum);
        assert!(!v.is_norm());
        assert_eq!(v.data, &[int(3), int(1), int(3), int(2)]);
        assert!(h.is_finite(e));
        assert_eq!(h.cardinality(e).ok(), Some(3));
        let n = h.norm(e).unwrap_or(Handle::EMPTY);
        assert!(h.view(n).is_norm());
        assert_eq!(h.view(n).data, &[int(1), int(2), int(3)]);
        let plain = h.set_from(&mut vec![int(1), int(2), int(3)]);
        let id_plain = h.intern(plain).unwrap_or(Handle::EMPTY);
        let (id, view) = h.intern_view(e).unwrap_or((Handle::EMPTY, Handle::EMPTY));
        assert_eq!(id, id_plain);
        assert_eq!(h.kind(view), Some(Kind::SetEnum));
        assert!(!h.view(view).is_norm());
        assert_eq!(h.intern(view).ok(), Some(id));
        assert!(!h.poisoned());
    }

    #[test]
    fn subset_cursor_is_in_canonical_order() {
        let mut h = heap(false);
        let base = h.set_from(&mut vec![int(1), int(2)]);
        let s = h.symbolic(Kind::Subset, 0, &[base]);
        let elems = h.elements(s).unwrap_or_default();
        assert_eq!(elems.len(), 4);
        for w in elems.windows(2) {
            assert_eq!(h.cmp(w[0], w[1]), Ordering::Less);
        }
    }
}
