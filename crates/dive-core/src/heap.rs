//! Value operations over both regions: views, hashing, equality, the
//! canonical order, builders, materialization of symbolic sets, and interning
//! by deep close.

use std::cmp::Ordering;
use std::sync::Arc;

use crate::Error;
use crate::arena::{Arena, Mark};
use crate::handle::{Handle, K_FCN, K_SET, K_SYM, Tag, fin, int_hash, mix2, mix64, scalar_hash};
use crate::node::{Kind, View, flags};
use crate::store::Store;

/// The largest set the core materializes.
pub const MAX_MATERIALIZE: usize = 1 << 24;

/// The most elements of a constant quantifier domain whose enumeration a
/// heap keeps ([`Heap::enumerate_constant`]).
pub const MAX_CONSTANT_DOMAIN: usize = 1 << 16;

/// The most transient handle words (node headers plus data) the kept
/// enumeration of one constant quantifier domain may add to a heap's store
/// ([`Heap::enumerate_constant`]): elements that are large compounds are
/// enumerated afresh instead of being retained.
pub const MAX_CONSTANT_DOMAIN_WORDS: usize = 1 << 20;

/// Maximum successful source nodes remembered by one local identity closure.
pub const MAX_LOCAL_CLOSE_ENTRIES: usize = 65_536;
/// Maximum aggregate live child-copy workspace in one local identity closure.
pub const MAX_LOCAL_CLOSE_SCRATCH_BYTES: usize = 64 * 1024 * 1024;

#[derive(Default)]
struct LocalClose {
    visited: std::collections::HashMap<Handle, Handle>,
    scratch: usize,
}

impl LocalClose {
    fn remember(&mut self, source: Handle, closed: Handle) -> Result<(), Error> {
        if !self.visited.contains_key(&source) {
            if self.visited.len() >= MAX_LOCAL_CLOSE_ENTRIES {
                return Err(Error::unsupported(
                    "candidate identity closure visited-node bound",
                ));
            }
            self.visited
                .try_reserve(1)
                .map_err(|_| Error::unsupported("candidate identity closure map allocation"))?;
        }
        self.visited.insert(source, closed);
        Ok(())
    }
}

static EMPTY_DATA: [Handle; 0] = [];

/// A worker's view of the value regions: the shared persistent store and its
/// own arena.
#[derive(Debug)]
pub struct Heap {
    pub(crate) store: Arc<Store>,
    pub(crate) arena: Arena,
    // Persistent source table, ground index, optional field and string result.
    // This heap owns the store that gives the table handle its identity.
    pub(crate) labels: [[Handle; 4]; 8],
    pub(crate) defer_reclamation: bool,
    /// Candidate expansion must not promote speculative results through caches.
    pub(crate) speculative: bool,
    /// Whether [`Heap::string`] refuses a text the shared table lacks
    /// ([`Heap::set_existing_strings_only`]).
    strings: StringMode,
    poisoned: std::cell::Cell<bool>,
    /// `-delta-hash`: recompute hashes in full instead of incrementally.
    pub(crate) full_hash: bool,
    /// The integer semantics (`int.rs`).
    pub(crate) int_mode: crate::int::IntMode,
    /// Interned enumerations of persistent finite symbolic sets, for
    /// [`Heap::materialize_cached`].
    materialized: std::collections::HashMap<Handle, Handle>,
    /// Persistent enumerations of constant quantifier domains, for
    /// [`Heap::enumerate_constant`]; `None` for one that is not kept.
    constant_domains: std::collections::HashMap<Handle, Option<Handle>>,
    /// Interned images of persistent values under one reduction's maps
    /// (`symmetry.rs`), keyed by map index and value, with the reduction
    /// they belong to.
    pub(crate) sym_images: crate::symmetry::ImageCache,
    work_control: Option<Arc<dyn crate::eval::WorkControl>>,
    work_steps: std::cell::Cell<u32>,
    work_interrupted: std::cell::Cell<bool>,
    /// This heap's copy of the string table's texts and ranks, by id,
    /// extended on a miss: string comparisons read it without the table's
    /// lock, which every worker would otherwise share.
    names: std::cell::RefCell<Names>,
    /// This heap's view of the store's deduplication tables, for interning
    /// a value already present without the shard lock.
    pub(crate) cache: crate::store::InternCache,
}

impl Drop for Heap {
    fn drop(&mut self) {
        self.store.flush_hits(&mut self.cache);
    }
}

/// A heap's lock-free copy of the shared string table
/// ([`crate::strings::Strings`]).
///
/// Ids never change their text, so the text copy never goes stale. Ranks
/// shift as strings are interned, but never reorder two ids the table had
/// already ranked together, so a rank copy that covers both ids of a
/// comparison orders them as the table does now.
#[derive(Debug, Default)]
struct Names {
    texts: Vec<Arc<str>>,
    ranks: Arc<[u32]>,
}

impl Names {
    /// Extends the copy when `id` is past it, taking the table's current
    /// ranks with the new texts.
    fn sync(&mut self, strings: &crate::strings::Strings, id: u32) {
        if (id as usize) < self.texts.len() {
            return;
        }
        let more = strings.texts_from(self.texts.len());
        if more.is_empty() {
            return;
        }
        self.texts.extend(more);
        self.ranks = strings.ranks();
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
            labels: [[Handle::EMPTY; 4]; 8],
            defer_reclamation: false,
            speculative: false,
            strings: StringMode::Intern,
            poisoned: std::cell::Cell::new(false),
            full_hash,
            int_mode,
            materialized: std::collections::HashMap::new(),
            constant_domains: std::collections::HashMap::new(),
            sym_images: crate::symmetry::ImageCache::default(),
            work_control: None,
            work_steps: std::cell::Cell::new(0),
            work_interrupted: std::cell::Cell::new(false),
            names: std::cell::RefCell::new(Names::default()),
            cache: crate::store::InternCache::default(),
        }
    }

    /// Compares two string or model-value ids in the table's order, as
    /// [`crate::strings::Strings::cmp`] does: by the ranks in this heap's
    /// copy when it ranks both ids, else from its copy of the texts.
    #[inline]
    pub(crate) fn str_cmp(&self, a: u32, b: u32) -> Ordering {
        if a == b {
            return Ordering::Equal;
        }
        {
            let names = self.names.borrow();
            if let (Some(x), Some(y)) = (names.ranks.get(a as usize), names.ranks.get(b as usize)) {
                return x.cmp(y);
            }
        }
        self.str_cmp_unranked(a, b)
    }

    /// [`Heap::str_cmp`] for an id outside this heap's rank copy: refreshes
    /// the copy, then compares by rank, by token or by text.
    #[inline(never)]
    fn str_cmp_unranked(&self, a: u32, b: u32) -> Ordering {
        let strings = self.store.strings();
        let mut names = self.names.borrow_mut();
        names.sync(strings, a.max(b));
        if let (Some(x), Some(y)) = (names.ranks.get(a as usize), names.ranks.get(b as usize)) {
            return x.cmp(y);
        }
        if strings.order() == crate::strings::StringOrder::Token {
            return a.cmp(&b);
        }
        match (names.texts.get(a as usize), names.texts.get(b as usize)) {
            (Some(x), Some(y)) => x.cmp(y),
            _ => a.cmp(&b),
        }
    }

    /// Whether the shared table has issued `id`.
    #[inline]
    pub(crate) fn known_name(&self, id: u32) -> bool {
        if (id as usize) < self.names.borrow().texts.len() {
            return true;
        }
        let mut names = self.names.borrow_mut();
        names.sync(self.store.strings(), id);
        (id as usize) < names.texts.len()
    }

    /// TLC's model-value type for a name, using this heap's append-only
    /// text copy. The rule counts UTF-16 units, as `ModelValue` does.
    pub(crate) fn model_type(&self, id: u32) -> Option<u16> {
        let mut names = self.names.borrow_mut();
        names.sync(self.store.strings(), id);
        let mut units = names.texts.get(id as usize)?.encode_utf16();
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

    /// The current transient allocation point.
    #[must_use]
    pub fn mark(&self) -> Mark {
        self.arena.mark()
    }

    /// Frees every transient allocated after `mark`. No handle allocated
    /// after the mark may be used afterwards, including one held in a frame
    /// slot or a thunk cache; such a use is detected as stale.
    pub fn rollback(&mut self, mark: Mark) {
        if !self.defer_reclamation {
            self.arena.rollback(mark);
        }
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
    /// needs a closed one.
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
            (v.flags & flags::LAZY) | (v.flags & flags::NESTED_VIEW),
        )
    }

    /// Whether a child is proved materialized without walking it: a known
    /// scalar, an eager big integer, or a node carrying [`flags::CONCRETE`].
    /// `false` proves nothing (`compare::materialized` decides).
    #[inline]
    fn concrete_child(&self, h: Handle) -> bool {
        if !h.is_compound() {
            return h
                .as_str_id()
                .or_else(|| h.as_model_id())
                .is_none_or(|id| self.known_name(id));
        }
        let v = self.view(h);
        v.flags & flags::CONCRETE != 0 || (v.kind == Kind::BigInt && !v.is_lazy())
    }

    /// [`flags::CONCRETE`] when every child is [`Heap::concrete_child`].
    fn concrete_bit(&self, children: &[Handle]) -> u8 {
        if children.iter().all(|&c| self.concrete_child(c)) {
            flags::CONCRETE
        } else {
            0
        }
    }

    /// Whether a value is proved materialized by a cached fact, without a
    /// walk: a boolean, an inline integer, a string or model value whose name
    /// this heap's [`Store`] issued, an eager big integer, or a persistent
    /// eager node carrying [`flags::CONCRETE`] (an eager `Set`, `IntFcn` or
    /// `Fcn` whose every descendant is materialized, as
    /// `compare::materialized` defines it). `false` proves nothing: snapshot
    /// restore, owned exchange and candidate decoding clear the bit, and
    /// [`Heap::materialized_within`] then decides.
    #[must_use]
    pub fn proved_concrete(&self, h: Handle) -> bool {
        if h.is_empty() || h.is_transient() {
            return false;
        }
        if !h.is_compound() {
            return h
                .as_str_id()
                .or_else(|| h.as_model_id())
                .is_none_or(|id| self.known_name(id));
        }
        let v = self.view(h);
        !v.is_lazy()
            && v.flags & flags::UNNORM == 0
            && (v.flags & flags::CONCRETE != 0 || v.kind == Kind::BigInt)
    }

    /// Whether every root is materialized as `compare::materialized`
    /// defines it (no lazy, symbolic or unnormalized node and no unknown name
    /// at any depth), decided by an iterative walk over at most `nodes`
    /// handles. Transient and empty handles decline. `false` proves nothing:
    /// an exhausted allowance or a failed scratch allocation also declines,
    /// and so does a heap that is poisoned when the walk ends.
    ///
    /// Without `name_bytes`, a node carrying [`flags::CONCRETE`] is trusted
    /// and not entered. With `name_bytes: Some(n)` every node is entered,
    /// and the text bytes of every string and model-value occurrence count
    /// against `n`.
    #[must_use]
    pub fn materialized_within(
        &self,
        roots: &[Handle],
        nodes: usize,
        name_bytes: Option<usize>,
    ) -> bool {
        let mut pending = Vec::new();
        if pending.try_reserve(roots.len()).is_err() {
            return false;
        }
        pending.extend_from_slice(roots);
        let (mut work, mut bytes) = (0usize, 0usize);
        while let Some(h) = pending.pop() {
            work += 1;
            if work > nodes || h.is_empty() || h.is_transient() {
                return false;
            }
            if !h.is_compound() {
                let Some(id) = h.as_str_id().or_else(|| h.as_model_id()) else {
                    continue;
                };
                if !self.known_name(id) {
                    return false;
                }
                if let Some(limit) = name_bytes {
                    let Some(len) = self.names.borrow().texts.get(id as usize).map(|t| t.len())
                    else {
                        return false;
                    };
                    bytes = bytes.saturating_add(len);
                    if bytes > limit {
                        return false;
                    }
                }
                continue;
            }
            let v = self.view(h);
            if v.is_lazy() || v.flags & flags::UNNORM != 0 {
                return false;
            }
            if name_bytes.is_none() && v.flags & flags::CONCRETE != 0 {
                continue;
            }
            match v.kind {
                Kind::BigInt => continue,
                Kind::Set | Kind::IntFcn | Kind::Fcn => {}
                _ => return false,
            }
            if v.data.len() > nodes.saturating_sub(work).saturating_sub(pending.len())
                || pending.try_reserve(v.data.len()).is_err()
            {
                return false;
            }
            pending.extend_from_slice(v.data);
        }
        !self.poisoned()
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

    /// The rank of a handle's class in the canonical order: model
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

    /// The canonical total order (the default profile's value order): model values by name, booleans,
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
            // By name, as strings.
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
        let concrete = self.concrete_bit(elems);
        self.arena.alloc(
            Kind::Set,
            concrete,
            0,
            set_hash(elems.len(), sum),
            sum,
            elems,
        )
    }

    /// The empty set.
    pub fn empty_set(&mut self) -> Handle {
        self.set_sorted(&[])
    }

    /// A function over `lo..lo+n-1` from normalized values.
    pub fn int_fcn(&mut self, lo: i64, vals: &[Handle]) -> Handle {
        self.int_fcn_as(lo, vals, 0)
    }

    pub(crate) fn int_fcn_as(&mut self, lo: i64, vals: &[Handle], view: u8) -> Handle {
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
        if fl & flags::LAZY == 0 {
            fl |= self.concrete_bit(vals);
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

    pub(crate) fn fcn_sorted_as(&mut self, keys: &[Handle], vals: &[Handle], view: u8) -> Handle {
        if let Some(lo) = contiguous_ints(keys) {
            return self.int_fcn(lo, vals);
        }
        let mut sum = 0u64;
        let mut fl = 0;
        for (&k, &v) in keys.iter().zip(vals) {
            let (kh, kf) = self.child(k);
            let (vh, vf) = self.child(v);
            // Only a lazy key is a misuse. A key may be a closed view (a
            // tuple of records an exact intern marked `NESTED_VIEW`), which
            // is then retained like a value's view.
            if kf & flags::LAZY != 0 {
                self.lazy_misuse();
            }
            sum = sum.wrapping_add(mix2(kh, vh));
            fl |= kf | vf;
        }
        let mut data = Vec::with_capacity(keys.len() * 2);
        data.extend_from_slice(keys);
        data.extend_from_slice(vals);
        if fl & flags::LAZY == 0 {
            fl |= self.concrete_bit(&data);
        }
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

    #[cfg(test)]
    pub(crate) fn canonical_test_set(&mut self, elems: &[Handle]) -> Result<Handle, Error> {
        crate::compare::require_materialized(self, elems)?;
        Ok(self.set_from(&mut elems.to_vec()))
    }

    /// A set enumeration `{e1, ...}` as TLC builds it before normalizing:
    /// the normalized, closed elements in construction order, duplicates kept, as a
    /// [`Kind::SetEnum`] view with [`flags::UNNORM`]. It is normalized,
    /// hashed and interned as the set of its elements; only membership and
    /// error paths that the contract names read its order.
    pub fn set_enum(&mut self, elems: &[Handle]) -> Handle {
        self.symbolic(Kind::SetEnum, 0, elems)
    }

    /// The same function with the entry at index `i` replaced, hashing in
    /// O(1) from the stored sum.
    #[allow(
        clippy::many_single_char_names,
        reason = "short names for the hash algebra"
    )]
    pub fn fcn_with(&mut self, f: Handle, i: usize, new: Handle) -> Handle {
        let v = self.view(f);
        let (kind, lo, sum, n, was_lazy) = (v.kind, v.lo, v.sum, v.len(), v.is_lazy());
        let was_concrete = v.flags & flags::CONCRETE != 0;
        // TLC's `RecordValue` and `TupleValue` EXCEPT keep their class.
        let record = v.flags & (flags::RECORD | flags::TUPLE);
        // The arena copies the finished row. A small table fits in the
        // stack scratch, avoiding a second allocation for one changed entry.
        let mut small = [Handle::EMPTY; 8];
        let mut large;
        let data = if let Some(scratch) = small.get_mut(..v.data.len()) {
            scratch.copy_from_slice(v.data);
            scratch
        } else {
            large = v.data.to_vec();
            large.as_mut_slice()
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
        let concrete = if was_concrete && fl & flags::LAZY == 0 && self.concrete_child(new) {
            flags::CONCRETE
        } else {
            0
        };
        self.arena.alloc(
            kind,
            fl | record | concrete,
            lo,
            fcn_hash(n, sum),
            sum,
            data,
        )
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
        if h.is_persistent() && !self.speculative {
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

    /// [`Heap::norm`] of a constant quantifier domain (a plan constant)
    /// that is enumerated once per evaluation of its quantifier. A
    /// persistent finite symbolic set of at most [`MAX_CONSTANT_DOMAIN`]
    /// elements (and [`MAX_CONSTANT_DOMAIN_WORDS`] new store words) is
    /// materialized once per heap and kept, when its
    /// enumeration is already in canonical persistent form (interning it
    /// exactly and canonically give the same handle; a read-only canonical
    /// form check runs first, so most rejected domains intern nothing). Its
    /// elements are then
    /// persistent, equal and identical to the ones a fresh enumeration
    /// builds. Any other set is normalized as [`Heap::norm`] does.
    ///
    /// # Errors
    /// As [`Heap::norm`].
    pub fn enumerate_constant(&mut self, h: Handle) -> Result<Handle, Error> {
        if !h.is_compound() || !h.is_persistent() || self.speculative {
            return self.norm(h);
        }
        let kept = self.constant_domains.get(&h).copied();
        if let Some(Some(m)) = kept {
            return Ok(m);
        }
        let v = self.view(h);
        if !v.kind.is_symbolic() || v.is_lazy() || !self.is_finite(h) {
            return Ok(h);
        }
        let m = self.materialize_fresh(h)?;
        if kept.is_some() {
            return Ok(m);
        }
        // Check, without interning, that the enumeration is small and
        // already in canonical form, so that a domain that will not be kept
        // leaves no interned copies behind in the store.
        let exact = if self.view(m).data.len() > MAX_CONSTANT_DOMAIN
            || !self.within_transient_words(m, MAX_CONSTANT_DOMAIN_WORDS)
            || !crate::compare::materialized(self, m, 0)?
        {
            None
        } else {
            match (self.intern_exact(m), self.intern(m)) {
                (Ok(exact), Ok(canonical)) if exact == canonical => Some(exact),
                _ => None,
            }
        };
        self.constant_domains.insert(h, exact);
        Ok(exact.unwrap_or(m))
    }

    /// Whether the transient nodes reachable from `h` (counted once per
    /// path, so shared nodes count more than once) hold at most `budget`
    /// handle words, headers included. Persistent children are not counted:
    /// interning reuses them.
    fn within_transient_words(&self, h: Handle, budget: usize) -> bool {
        let mut left = budget;
        let mut stack = vec![h];
        while let Some(n) = stack.pop() {
            if !n.is_compound() || !n.is_transient() {
                continue;
            }
            let data = self.view(n).data;
            let Some(rest) = left.checked_sub(1 + data.len()) else {
                return false;
            };
            left = rest;
            stack.extend(data.iter().copied().filter(|d| d.is_transient()));
        }
        true
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

    /// Read-only `Value.isEmpty` on concrete explicit/interval/product axes.
    /// Unknown axes decline in source order; later emptiness cannot waive them.
    /// Returns visited work for callers that also own a finite fuel budget.
    pub(crate) fn product_empty(&self, h: Handle) -> Result<(Option<bool>, usize), Error> {
        let mut work = 0;
        let result = self.concrete_product_empty(h, 0, &mut work)?;
        Ok((result, work))
    }

    fn concrete_product_empty(
        &self,
        h: Handle,
        depth: u8,
        work: &mut usize,
    ) -> Result<Option<bool>, Error> {
        self.work_poll()?;
        *work = work.saturating_add(1);
        if depth >= 64 || !h.is_compound() {
            return Ok(None);
        }
        let view = self.view(h);
        match view.kind {
            Kind::Set | Kind::SetEnum => Ok(Some(view.data.is_empty())),
            Kind::Interval => Ok(view.data.first().and_then(|h| h.as_int()).and_then(|hi| {
                if hi < view.lo {
                    Some(true)
                } else {
                    hi.checked_sub(view.lo)
                        .and_then(|n| n.checked_add(1))
                        .filter(|&n| n <= i64::from(i32::MAX))
                        .map(|_| false)
                }
            })),
            Kind::Product => {
                for &axis in view.data {
                    match self.concrete_product_empty(axis, depth + 1, work)? {
                        Some(false) => {}
                        empty => return Ok(empty),
                    }
                }
                Ok(Some(false))
            }
            _ => Ok(None),
        }
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
        let v = self.view(h);
        let (kind, lo, data) = (v.kind, v.lo, v.data.to_vec());
        Ok(match kind {
            Kind::Set => Cursor::Array { set: h, i: 0 },
            Kind::Interval => Cursor::Range {
                cur: lo,
                hi: data[0].as_int().unwrap_or(lo - 1),
            },
            Kind::Product => {
                // SetOfTuplesValue.elements first checks source-order emptiness.
                // Do this at materialization, including compound components,
                // before normalizing any axis or requiring later axes be sets.
                if self.product_empty(h)?.0 == Some(true) {
                    return Ok(Cursor::Array {
                        set: self.empty_set(),
                        i: 0,
                    });
                }
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

    /// Selects speculative candidate evaluation, returning the previous mode.
    ///
    /// In this mode evaluator memo sites and materialization caches do not
    /// promote results. Explicit `intern` calls retain their persistent contract.
    pub fn set_speculative(&mut self, enabled: bool) -> bool {
        std::mem::replace(&mut self.speculative, enabled)
    }

    /// Whether candidate evaluation is keeping identities local to this Arena.
    #[must_use]
    pub const fn is_speculative(&self) -> bool {
        self.speculative
    }

    /// Interns for an ordinary run, or closes locally during candidate expansion.
    ///
    /// # Errors
    /// As [`Self::intern`] or [`Self::close_identity_local`].
    pub fn state_identity(&mut self, h: Handle) -> Result<Handle, Error> {
        if self.speculative {
            self.close_identity_local(h)
        } else {
            self.intern(h)
        }
    }

    /// Closes a mathematical identity in the local Arena without Store promotion.
    ///
    /// This performs the same demanded normalization as [`Self::intern`], but
    /// returns values valid only until the owning Arena is rolled back or reset.
    /// No persistent-handle certificate or interning cache is populated.
    ///
    /// # Errors
    /// An unclosed lazy value, failed normalization, or a too-deep candidate.
    pub fn close_identity_local(&mut self, h: Handle) -> Result<Handle, Error> {
        let h = self.close_local(h, 0, &mut LocalClose::default())?;
        crate::compare::require_materialized(self, &[h])?;
        Ok(h)
    }

    /// Closes related roots with one bounded, operation-local sharing map.
    ///
    /// No result or source mapping survives this call. Original values retain
    /// their symbolic representations; returned roots are Arena-local.
    ///
    /// # Errors
    /// As [`Self::close_identity_local`], or root-row workspace exhaustion.
    pub fn close_identity_row_local(&mut self, roots: &[Handle]) -> Result<Vec<Handle>, Error> {
        if std::mem::size_of_val(roots) > MAX_LOCAL_CLOSE_SCRATCH_BYTES {
            return Err(Error::unsupported(
                "candidate identity root-row workspace bound",
            ));
        }
        let mut out = Vec::new();
        out.try_reserve_exact(roots.len())
            .map_err(|_| Error::unsupported("candidate identity root-row allocation"))?;
        let mut close = LocalClose::default();
        for &root in roots {
            let root = self.close_local(root, 0, &mut close)?;
            crate::compare::require_materialized(self, &[root])?;
            out.push(root);
        }
        Ok(out)
    }

    fn close_local(
        &mut self,
        source: Handle,
        depth: usize,
        close: &mut LocalClose,
    ) -> Result<Handle, Error> {
        let h = source;
        if !h.is_compound() {
            return Ok(h);
        }
        if let Some(&closed) = close.visited.get(&h) {
            return Ok(closed);
        }
        if depth >= 128 {
            return Err(Error::unsupported("candidate identity nesting exceeds 128"));
        }
        self.work_poll()?;
        if let Some(&closed) = close.visited.get(&h) {
            close.remember(source, closed)?;
            return Ok(closed);
        }
        let view = self.view(h);
        if view.is_lazy() {
            return Err(Error::internal("closing an unclosed lazy candidate value"));
        }
        if view.kind.is_symbolic() && self.is_finite(h) {
            let materialized = self.materialize(h)?;
            let closed = self.close_local(materialized, depth + 1, close)?;
            close.remember(source, closed)?;
            close.remember(h, closed)?;
            return Ok(closed);
        }
        if view.kind.is_symbolic() {
            return Err(Error::unsupported(
                "mathematical identity of a symbolic state value",
            ));
        }
        if view.kind == Kind::Fcn {
            // Closing a symbolic key could change its order or collapse two
            // keys. A sorted function cannot silently inherit either change.
            crate::compare::require_materialized(self, &view.data[..view.data.len() / 2])?;
        }
        // Constants can retain symbolic descendants even without view flags.
        if h.is_persistent()
            && view.flags & flags::VIEW == 0
            && crate::compare::materialized(self, h, 0)?
        {
            return Ok(h);
        }
        let lo = if view.kind == Kind::Set { 0 } else { view.lo };
        let bytes = std::mem::size_of_val(view.data);
        if bytes > MAX_LOCAL_CLOSE_SCRATCH_BYTES.saturating_sub(close.scratch) {
            return Err(Error::unsupported(
                "candidate identity closure child workspace bound",
            ));
        }
        let mut children = Vec::new();
        children
            .try_reserve_exact(view.data.len())
            .map_err(|_| Error::unsupported("candidate identity closure child allocation"))?;
        let bytes = children.capacity() * size_of::<Handle>();
        if bytes > MAX_LOCAL_CLOSE_SCRATCH_BYTES.saturating_sub(close.scratch) {
            return Err(Error::unsupported(
                "candidate identity closure child workspace bound",
            ));
        }
        close.scratch += bytes;
        for chunk in view.data.chunks(64) {
            self.work_poll()?;
            children.extend_from_slice(chunk);
        }
        for child in &mut children {
            *child = self.close_local(*child, depth + 1, close)?;
        }
        let unchanged = self.view(h).data == children;
        let closed =
            if unchanged && self.view(h).flags & !flags::CONCRETE == 0 && self.view(h).lo == lo {
                h
            } else if unchanged && h.is_transient() {
                self.arena
                    .project(h, 0, lo)
                    .ok_or_else(|| Error::internal("candidate identity projection owner"))?
            } else {
                // Materializing a child changes its value hash and can coalesce
                // equal set members. Recompute the parent's structure and hash.
                self.rebuild(h, &children)
            };
        close.scratch -= bytes;
        close.remember(source, closed)?;
        close.remember(h, closed)?;
        Ok(closed)
    }

    /// Interns a value into the persistent store, deep-closing it: children
    /// first, finite symbolic sets materialized.
    ///
    /// # Errors
    /// A set too large to materialize.
    pub fn intern(&mut self, h: Handle) -> Result<Handle, Error> {
        if crate::compare::materialized(self, h, 0)? {
            self.intern_materialized(h)
        } else {
            let h = self.close_identity_local(h)?;
            self.intern_materialized(h)
        }
    }

    fn intern_materialized(&mut self, h: Handle) -> Result<Handle, Error> {
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
                            *d = self.intern_materialized(*d)?;
                        }
                        let concrete = self.concrete_identity(kind, &data);
                        return Ok(self.store.intern(
                            &mut self.cache,
                            kind,
                            concrete,
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
                    return self.intern_materialized(m);
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
            let p = self.intern_materialized(m)?;
            self.arena.interned[idx] = p;
            return Ok(p);
        }
        let (kind, lo, hash, sum) = (
            v.kind,
            if v.kind == Kind::Set { 0 } else { v.lo },
            v.hash,
            v.sum,
        );
        let mut data = v.data.to_vec();
        for d in &mut data {
            if d.is_compound() {
                *d = self.intern_materialized(*d)?;
            }
        }
        let concrete = self.concrete_identity(kind, &data);
        let p = self
            .store
            .intern(&mut self.cache, kind, concrete, lo, hash, sum, &data);
        self.arena.interned[idx] = p;
        Ok(p)
    }

    /// [`flags::CONCRETE`] for an eager canonical identity node.
    fn concrete_identity(&self, kind: Kind, data: &[Handle]) -> u8 {
        if matches!(kind, Kind::Set | Kind::IntFcn | Kind::Fcn) {
            self.concrete_bit(data)
        } else {
            0
        }
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
    /// Step-predicate memo sites use it for their keys and results;
    /// graph capture uses it to retain original
    /// target representations independently of canonical state identities.
    ///
    /// # Errors
    /// `h` is lazy (holds a closure only the evaluator can force).
    pub fn intern_exact(&mut self, h: Handle) -> Result<Handle, Error> {
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

    pub(crate) fn set_work_control(&mut self, control: Option<Arc<dyn crate::eval::WorkControl>>) {
        self.work_control = control;
    }

    pub(crate) fn mark_work_interrupted(&self) {
        self.work_interrupted.set(true);
    }

    /// Whether this machine reached its caller-owned cooperative stop.
    #[must_use]
    pub fn work_interrupted(&self) -> bool {
        self.work_interrupted.get()
    }

    pub(crate) fn work_poll(&self) -> Result<(), Error> {
        let steps = self.work_steps.get();
        self.work_steps.set(steps.wrapping_add(1));
        if steps.trailing_zeros() >= 6 && self.work_control.as_ref().is_some_and(|c| !c.proceed()) {
            self.mark_work_interrupted();
            return Err(Error::stop());
        }
        Ok(())
    }

    /// The string id of `text`.
    ///
    /// # Errors
    /// The string table is full, or the text is new while
    /// [`Heap::set_existing_strings_only`] holds (unsupported).
    pub fn string(&self, text: &str) -> Result<Handle, Error> {
        if self.strings == StringMode::ExistingOnly {
            return self
                .store
                .strings()
                .lookup(text)
                .map(Handle::string)
                .ok_or_else(|| {
                    Error::unsupported(
                        "a string this optional evaluation would add to the shared string table",
                    )
                });
        }
        Ok(Handle::string(self.store.strings().intern(text)?))
    }

    /// Makes [`Heap::string`] refuse any text the shared table does not
    /// already hold, returning the previous mode. An isolated optional
    /// machine uses it so that skipping its work elsewhere cannot change
    /// later string tokens (string order under `--compat=tlc`).
    ///
    /// ```
    /// # fn example(heap: &mut dive_core::Heap) {
    /// let previous = heap.set_existing_strings_only(true);
    /// heap.set_existing_strings_only(previous);
    /// # }
    /// ```
    pub fn set_existing_strings_only(&mut self, enabled: bool) -> bool {
        let mode = if enabled {
            StringMode::ExistingOnly
        } else {
            StringMode::Intern
        };
        std::mem::replace(&mut self.strings, mode) == StringMode::ExistingOnly
    }
}

/// Whether [`Heap::string`] may add a text to the shared string table.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StringMode {
    /// Assign an id to a new text.
    Intern,
    /// Refuse a text the table lacks.
    ExistingOnly,
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
impl Heap {
    /// A persistent copy of a transient node without [`flags::CONCRETE`], as
    /// snapshot restore and owned exchange produce it.
    pub(crate) fn unmarked_copy(&mut self, t: Handle) -> Handle {
        let v = self.view(t);
        let (kind, fl, lo, hash, sum) = (v.kind, v.flags & !flags::CONCRETE, v.lo, v.hash, v.sum);
        let data = v.data.to_vec();
        self.store
            .intern(&mut self.cache, kind, fl, lo, hash, sum, &data)
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
    fn mathematical_state_identity_refuses_nested_symbolic_values() {
        let mut h = heap(false);
        let empty = h.empty_set();
        let naturals = h.symbolic(Kind::Nat, 0, &[]);
        let product = h.symbolic(Kind::Product, 0, &[empty, naturals]);
        let range = h.interval(1, 1);
        let sequence = h.symbolic(Kind::SeqSet, 0, &[range]);
        for symbolic in [sequence, product] {
            let tuple = h.tuple(&[symbolic]);
            let function_key = h.fcn_sorted(&[symbolic], &[int(1)]);
            let function_value = h.fcn_sorted(&[int(0)], &[symbolic]);
            let singleton = h.set_sorted(&[tuple]);
            for value in [symbolic, tuple, function_key, function_value, singleton] {
                let before = h.store().stats().nodes;
                assert!(
                    matches!(h.intern(value), Err(error) if error.kind() == crate::ErrorKind::Unsupported)
                );
                assert!(
                    matches!(h.close_identity_local(value), Err(error) if error.kind() == crate::ErrorKind::Unsupported)
                );
                assert!(
                    matches!(h.close_identity_row_local(&[value]), Err(error) if error.kind() == crate::ErrorKind::Unsupported)
                );
                assert_eq!(h.store().stats().nodes, before);
            }
        }
    }

    #[test]
    fn product_materialization_checks_empty_axes_before_normalization() -> Result<(), Error> {
        for semantics in [crate::Semantics::default(), crate::Semantics::tlc()] {
            let mut h = Heap::new(
                Arc::new(Store::with_semantics(true, semantics)),
                false,
                false,
            );
            let empty_interval = h.interval(3, 2);
            let mixed = h.set_from(&mut vec![int(0), Handle::TRUE]);
            let product = h.symbolic(Kind::Product, 0, &[empty_interval, mixed, int(0)]);
            let nested = h.symbolic(Kind::Product, 0, &[product, mixed]);
            for value in [product, nested] {
                let materialized = h.materialize(value)?;
                assert_eq!(h.view(materialized).kind, Kind::Set);
                assert_eq!(h.view(materialized).data, []);
            }
            // An invalid scalar axis still refuses when it precedes emptiness.
            let earlier_scalar = h.symbolic(Kind::Product, 0, &[int(0), empty_interval]);
            assert!(h.materialize(earlier_scalar).is_err());
            let one = h.set_from(&mut vec![int(0)]);
            let demanded = h.symbolic(Kind::Product, 0, &[one, mixed]);
            let materialized = h.materialize(demanded)?;
            assert_eq!(h.view(materialized).data.len(), 2);
            assert!(!h.poisoned());
        }
        Ok(())
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

    /// Strings the table has not ranked yet compare by text, against ranked
    /// and unranked strings alike, as the table orders them.
    #[test]
    fn string_comparison_covers_strings_past_the_ranks() {
        let h = heap(false);
        let t = h.store().strings();
        let id = |s: &str| t.intern(s).unwrap_or(u32::MAX);
        let ranked: Vec<u32> = (0..1500).map(|n| id(&format!("k{n:04}"))).collect();
        assert_eq!(h.str_cmp(ranked[1], ranked[0]), Ordering::Greater);
        let late = [id("k0500x"), id("a"), id("zz")];
        assert!(t.ranks().len() < t.len(), "the late strings stay unranked");
        for &x in late.iter().chain(&ranked[495..505]) {
            for &y in late.iter().chain(&ranked[495..505]) {
                assert_eq!(h.str_cmp(x, y), t.cmp(x, y), "{x} {y}");
            }
        }
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

    /// A function key may be a closed view: a tuple of records that an
    /// exact intern (graph capture for liveness) marked `NESTED_VIEW`, as
    /// `KeyValueStore`'s `PermSeqs` over its logged operations builds.
    /// Only a lazy key is a misuse.
    #[test]
    fn a_closed_view_is_a_function_key() {
        let mut h = heap(false);
        let a = h.string("a").unwrap_or(Handle::EMPTY);
        let rec = h.record(&[a], &[int(1)]);
        let seq = h.tuple_literal(&[rec]);
        let key = h.intern_exact(seq).unwrap_or(Handle::EMPTY);
        assert_ne!(h.view(key).flags & flags::NESTED_VIEW, 0);
        let f = h.fcn_sorted(&[key, a], &[int(1), int(2)]);
        assert!(!h.poisoned());
        assert_ne!(h.view(f).flags & flags::NESTED_VIEW, 0, "the view is kept");
        let plain = h.tuple(&[rec]);
        let g = h.fcn_sorted(&[plain, a], &[int(1), int(2)]);
        assert!(h.eq(f, g));
        let dom = h.interval(1, 2);
        let lazy = h.set_pred(0, dom, &[]);
        let _ = h.fcn_sorted(&[lazy], &[int(1)]);
        assert!(h.poisoned(), "a lazy key is still a misuse");
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

    #[test]
    fn constant_domains_are_enumerated_once_into_equal_persistent_elements() {
        let mut h = heap(false);
        let ints = h.interval(1, 2);
        let names = h.set_from(&mut vec![int(7), int(8)]);
        let product = h.symbolic(Kind::Product, 0, &[ints, names]);
        let subset = h.symbolic(Kind::Subset, 0, &[product]);
        let domain = h.intern_exact(subset).unwrap_or(Handle::EMPTY);
        assert!(domain.is_persistent());
        let fresh = h.norm(domain).unwrap_or(Handle::EMPTY);
        let kept = h.enumerate_constant(domain).unwrap_or(Handle::EMPTY);
        assert!(kept.is_persistent());
        assert_eq!(h.enumerate_constant(domain).ok(), Some(kept));
        let fresh_elems = h.elements(fresh).unwrap_or_default();
        let kept_elems = h.elements(kept).unwrap_or_default();
        assert_eq!(kept_elems.len(), 16);
        assert_eq!(fresh_elems.len(), kept_elems.len());
        for (&a, &b) in fresh_elems.iter().zip(&kept_elems) {
            assert!(b.is_persistent());
            assert_eq!(h.cmp(a, b), Ordering::Equal);
            assert_eq!(h.intern(a).ok(), Some(b));
        }
        // An enumeration above the bound is built afresh every time.
        let big = h.interval(1, 17);
        let big = h.symbolic(Kind::Subset, 0, &[big]);
        let big = h.intern_exact(big).unwrap_or(Handle::EMPTY);
        for _ in 0..2 {
            let m = h.enumerate_constant(big).unwrap_or(Handle::EMPTY);
            assert!(m.is_transient());
            assert_eq!(h.view(m).data.len(), 1 << 17);
        }
        // The word budget counts transient headers and data, not
        // persistent children.
        let fresh = h.norm(domain).unwrap_or(Handle::EMPTY);
        assert!(h.within_transient_words(fresh, 1 << 12));
        // The top node alone holds 1 + 16 words.
        assert!(!h.within_transient_words(fresh, 16));
        assert!(h.within_transient_words(kept, 0));
        // Non-symbolic and transient sets are normalized as `norm` does.
        let set = h.set_from(&mut vec![int(1)]);
        assert_eq!(h.enumerate_constant(set).ok(), Some(set));
        assert!(!h.poisoned());
    }

    fn unmarked(h: &mut Heap, t: Handle) -> Handle {
        h.unmarked_copy(t)
    }

    #[test]
    fn concrete_proofs_trust_the_bit_and_the_bounded_walk_decides_without_it() {
        let mut h = heap(false);
        let name = h.string("abcd").unwrap_or(Handle::EMPTY);
        let model = Handle::model(h.store().strings().intern("m").unwrap_or(0));
        for scalar in [Handle::TRUE, int(-3), name, model] {
            assert!(h.proved_concrete(scalar));
            assert!(h.materialized_within(&[scalar], 1, None));
        }
        let unknown = Handle::string(1 << 20);
        for refused in [Handle::EMPTY, unknown, Handle::model(1 << 20)] {
            assert!(!h.proved_concrete(refused));
            assert!(!h.materialized_within(&[refused], 8, None));
        }
        // Eager builders and exact interning keep the cached proof.
        let tuple = h.tuple_literal(&[name, model]);
        let set = h.set_from(&mut vec![tuple, int(1), model]);
        assert!(!h.proved_concrete(set), "a transient is never proved");
        assert!(!h.materialized_within(&[set], 64, None));
        let marked = h.intern_exact(set).unwrap_or(Handle::EMPTY);
        assert!(h.proved_concrete(marked));
        assert!(h.materialized_within(&[marked], 1, None));
        // With a name allowance every node is entered and every occurrence counts.
        assert!(h.materialized_within(&[marked], 64, Some(6)));
        assert!(!h.materialized_within(&[marked], 64, Some(5)));
        assert!(!h.materialized_within(&[marked], 3, Some(64)));
        // Without the bit the walk decides, within its node allowance.
        let inner = h.tuple_literal(&[int(7), name]);
        let inner = unmarked(&mut h, inner);
        let outer = h.set_from(&mut vec![inner, Handle::FALSE]);
        let outer = unmarked(&mut h, outer);
        assert_eq!(h.view(outer).flags & flags::CONCRETE, 0);
        assert!(!h.proved_concrete(outer));
        assert!(h.materialized_within(&[outer], 5, None));
        assert!(!h.materialized_within(&[outer], 4, None));
        // Symbolic, unnormalized and lazy nodes decline at any depth.
        let interval = h.interval(1, 3);
        let raw = h.set_enum(&[int(2), int(1)]);
        let raw = h.intern_exact(raw).unwrap_or(Handle::EMPTY);
        for symbolic in [interval, raw] {
            let symbolic = h.intern_exact(symbolic).unwrap_or(Handle::EMPTY);
            let nested = h.tuple(&[symbolic]);
            let nested = unmarked(&mut h, nested);
            for value in [symbolic, nested] {
                assert!(!h.proved_concrete(value));
                assert!(!h.materialized_within(&[value], 64, None));
            }
        }
        let big = h.int_from_big(&(num_bigint::BigInt::from(1) << 80u32));
        if big.is_compound() {
            let big = h.intern_exact(big).unwrap_or(Handle::EMPTY);
            assert!(h.proved_concrete(big));
            assert!(h.materialized_within(&[big], 1, None));
        }
        assert!(!h.poisoned());
        let _ = h.view(Handle::EMPTY);
        assert!(
            !h.materialized_within(&[int(1)], 1, None),
            "current poison declines"
        );
    }
}
