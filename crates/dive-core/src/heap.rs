//! Value operations over both regions: views, hashing (`03-value-model.md`
//! §5), equality, the canonical order (§7), builders, materialization of
//! symbolic sets, and interning by deep close (§3.3).

use std::cmp::Ordering;
use std::sync::Arc;

use crate::Error;
use crate::arena::Arena;
use crate::handle::{Handle, K_FCN, K_SET, K_SYM, Tag, fin, int_hash, mix2, mix64, scalar_hash};
use crate::node::{Kind, View};
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
    /// Interned enumerations of persistent finite symbolic sets, for
    /// [`Heap::materialize_cached`].
    materialized: std::collections::HashMap<Handle, Handle>,
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
        Self {
            store,
            arena: Arena::new(heap_arena),
            poisoned: std::cell::Cell::new(false),
            full_hash,
            materialized: std::collections::HashMap::new(),
        }
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
    }

    /// Whether a stale or foreign handle was read: an internal error.
    #[must_use]
    pub fn poisoned(&self) -> bool {
        self.poisoned.get() || self.arena.stale() || self.store.poisoned()
    }

    /// The view of a compound handle. A stale or scalar handle gives an empty
    /// set view and poisons the heap.
    #[must_use]
    pub fn view(&self, h: Handle) -> View<'_> {
        match h.tag() {
            Tag::Persistent => self.store.node(h).view(),
            Tag::Transient => match self.arena.lookup(h) {
                Some(v) => v,
                None => self.poison_view(),
            },
            _ => self.poison_view(),
        }
    }

    fn poison_view(&self) -> View<'_> {
        self.poisoned.set(true);
        View {
            kind: Kind::Set,
            lo: 0,
            hash: 0,
            sum: 0,
            data: &EMPTY_DATA,
        }
    }

    /// The kind of a compound, `None` for a scalar.
    #[must_use]
    pub fn kind(&self, h: Handle) -> Option<Kind> {
        h.is_compound().then(|| self.view(h).kind)
    }

    /// The value hash. Finite symbolic sets must be normalized first.
    #[must_use]
    pub fn hash(&self, h: Handle) -> u64 {
        if let Some(s) = scalar_hash(h) {
            return s;
        }
        if self.full_hash {
            return self.deep_hash(h);
        }
        self.view(h).hash
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
        if a.is_persistent() && b.is_persistent() && self.store.hashcons() {
            return false;
        }
        let (va, vb) = (self.view(a), self.view(b));
        if va.hash != vb.hash
            || va.kind != vb.kind
            || va.lo != vb.lo
            || va.data.len() != vb.data.len()
        {
            return false;
        }
        va.data.iter().zip(vb.data).all(|(&x, &y)| self.eq(x, y))
    }

    fn scalar_rank(h: Handle) -> u8 {
        match h.tag() {
            Tag::Bool => 1,
            Tag::Int => 2,
            Tag::Str => 3,
            Tag::Persistent | Tag::Transient => 4,
            Tag::Empty => 0,
        }
    }

    fn kind_rank(k: Kind) -> u8 {
        match k {
            Kind::Set => 4,
            Kind::IntFcn | Kind::Fcn => 5,
            _ => 6,
        }
    }

    /// The canonical total order (D-010): booleans, integers, strings by
    /// text, finite sets by size then elements, functions by size, domain
    /// then values, then symbolic sets structurally. Consistent with
    /// [`Heap::eq`] on normalized handles.
    #[must_use]
    pub fn cmp(&self, a: Handle, b: Handle) -> Ordering {
        if a == b {
            return Ordering::Equal;
        }
        let (ra, rb) = (Self::scalar_rank(a), Self::scalar_rank(b));
        if ra != rb {
            return ra.cmp(&rb);
        }
        match a.tag() {
            Tag::Bool => a.as_bool().cmp(&b.as_bool()),
            Tag::Int => a.as_int().cmp(&b.as_int()),
            Tag::Str => match (a.as_str_id(), b.as_str_id()) {
                (Some(x), Some(y)) => self.store.strings().cmp(x, y),
                _ => Ordering::Equal,
            },
            Tag::Persistent | Tag::Transient => self.cmp_compound(a, b),
            Tag::Empty => Ordering::Equal,
        }
    }

    fn cmp_compound(&self, a: Handle, b: Handle) -> Ordering {
        let (va, vb) = (self.view(a), self.view(b));
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

    /// A set from sorted, distinct, normalized elements.
    pub fn set_sorted(&mut self, elems: &[Handle]) -> Handle {
        let sum = elems
            .iter()
            .fold(0u64, |a, &e| a.wrapping_add(self.hash(e)));
        self.arena
            .alloc(Kind::Set, 0, set_hash(elems.len(), sum), sum, elems)
    }

    /// The empty set.
    pub fn empty_set(&mut self) -> Handle {
        self.set_sorted(&[])
    }

    /// A function over `lo..lo+n-1` from normalized values.
    pub fn int_fcn(&mut self, lo: i64, vals: &[Handle]) -> Handle {
        let lo = if vals.is_empty() { 1 } else { lo };
        let mut sum = 0u64;
        for (i, &v) in vals.iter().enumerate() {
            #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
            let k = lo + i as i64;
            sum = sum.wrapping_add(mix2(int_hash(k), self.hash(v)));
        }
        self.arena
            .alloc(Kind::IntFcn, lo, fcn_hash(vals.len(), sum), sum, vals)
    }

    /// A tuple (a function over `1..n`).
    pub fn tuple(&mut self, vals: &[Handle]) -> Handle {
        self.int_fcn(1, vals)
    }

    /// A function from sorted, distinct, normalized keys and their values,
    /// in canonical form.
    pub fn fcn_sorted(&mut self, keys: &[Handle], vals: &[Handle]) -> Handle {
        if let Some(lo) = contiguous_ints(keys) {
            return self.int_fcn(lo, vals);
        }
        let mut sum = 0u64;
        for (&k, &v) in keys.iter().zip(vals) {
            sum = sum.wrapping_add(mix2(self.hash(k), self.hash(v)));
        }
        let mut data = Vec::with_capacity(keys.len() * 2);
        data.extend_from_slice(keys);
        data.extend_from_slice(vals);
        self.arena
            .alloc(Kind::Fcn, 0, fcn_hash(keys.len(), sum), sum, &data)
    }

    /// A function from `(key, value)` pairs with distinct keys, any order.
    pub fn fcn_from_pairs(&mut self, pairs: &mut [(Handle, Handle)]) -> Handle {
        pairs.sort_unstable_by(|a, b| self.cmp(a.0, b.0));
        let keys: Vec<Handle> = pairs.iter().map(|p| p.0).collect();
        let vals: Vec<Handle> = pairs.iter().map(|p| p.1).collect();
        self.fcn_sorted(&keys, &vals)
    }

    /// The same function with the entry at index `i` replaced, hashing in
    /// O(1) from the stored sum (S2).
    #[allow(
        clippy::many_single_char_names,
        reason = "short names for the hash algebra"
    )]
    pub fn fcn_with(&mut self, f: Handle, i: usize, new: Handle) -> Handle {
        let v = self.view(f);
        let (kind, lo, sum, n) = (v.kind, v.lo, v.sum, v.len());
        let mut data = v.data.to_vec();
        let (key_hash, slot) = match kind {
            Kind::IntFcn => {
                #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
                let k = lo + i as i64;
                (int_hash(k), i)
            }
            _ => (self.hash(data[i]), n + i),
        };
        let old = data[slot];
        data[slot] = new;
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
                    _ => (self.hash(data[j]), data[n + j]),
                };
                s = s.wrapping_add(mix2(kh, self.hash(x)));
            }
            s
        } else {
            sum.wrapping_sub(mix2(key_hash, self.hash(old)))
                .wrapping_add(mix2(key_hash, self.hash(new)))
        };
        self.arena.alloc(kind, lo, fcn_hash(n, sum), sum, &data)
    }

    /// A symbolic set node.
    pub fn symbolic(&mut self, kind: Kind, lo: i64, data: &[Handle]) -> Handle {
        let mut h = mix64(K_SYM ^ (kind as u64) ^ lo.cast_unsigned().rotate_left(17));
        for &d in data {
            h = mix2(h, self.hash(d));
        }
        self.arena.alloc(kind, lo, h, 0, data)
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
            Kind::Set | Kind::IntFcn | Kind::Fcn | Kind::Interval => true,
            Kind::Nat | Kind::Int => false,
            Kind::SeqSet => self.view_is_empty_set(v.data[0]),
            Kind::Subset | Kind::FcnSet | Kind::Product => {
                v.data.iter().all(|&d| self.is_finite(d))
            }
            Kind::RecSet => v.data[v.data.len() / 2..]
                .iter()
                .all(|&d| self.is_finite(d)),
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
        let k = self.view(h).kind;
        if !k.is_symbolic() || !self.is_finite(h) {
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
            Kind::Nat | Kind::Int | Kind::SeqSet => {
                return Err(Error::eval("attempted to enumerate an infinite set"));
            }
            Kind::IntFcn | Kind::Fcn => {
                return Err(Error::eval("attempted to enumerate a function as a set"));
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
        match h.tag() {
            Tag::Transient => {}
            Tag::Persistent => {
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
        if v.kind.is_symbolic() && self.is_finite(h) {
            let m = self.materialize(h)?;
            let p = self.intern(m)?;
            self.arena.interned[idx] = p;
            return Ok(p);
        }
        let (kind, lo, hash, sum) = (v.kind, v.lo, v.hash, v.sum);
        let mut data = v.data.to_vec();
        for d in &mut data {
            if d.is_compound() {
                *d = self.intern(*d)?;
            }
        }
        let p = self.store.intern(kind, lo, hash, sum, &data);
        self.arena.interned[idx] = p;
        Ok(p)
    }

    /// Interns a constant, keeping a top-level symbolic set symbolic (its
    /// components are interned normally), so that membership stays symbolic.
    ///
    /// # Errors
    /// As [`Heap::intern`].
    pub fn intern_const(&mut self, h: Handle) -> Result<Handle, Error> {
        if !h.is_transient() {
            return Ok(h);
        }
        let v = self.view(h);
        if !v.kind.is_symbolic() {
            return self.intern(h);
        }
        let (kind, lo, hash, sum) = (v.kind, v.lo, v.hash, v.sum);
        let mut data = v.data.to_vec();
        for d in &mut data {
            *d = self.intern_const(*d)?;
        }
        Ok(self.store.intern(kind, lo, hash, sum, &data))
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
fn fcn_key(v: &View<'_>, i: usize) -> Handle {
    match v.kind {
        #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
        Kind::IntFcn => Handle::int(v.lo + i as i64).unwrap_or(Handle::EMPTY),
        _ => v.data[i],
    }
}

/// The lower bound if `keys` are the consecutive integers `lo, lo+1, ...`.
fn contiguous_ints(keys: &[Handle]) -> Option<i64> {
    let lo = keys.first()?.as_int()?;
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
