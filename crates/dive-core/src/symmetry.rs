//! Symmetry reduction (`SYMMETRY`): the state key of a state is the key of
//! its canonical image, the least of its images under the closure's
//! permutations of model values, compared variable by variable in the
//! run's value order. The explored state stays the concrete one; only keys
//! change, as in the reference engine
//! (`crates/dive-model/src/symmetry/prepare.rs`, `Reduction::canonical_values`).
//!
//! The default profile's canonicalization (D-010) compares in the canonical
//! value order, variables in declaration order. TLC's (`--compat=tlc`,
//! [`Symmetry::tlc`]) compares with TLC's `compareTo`, variables in TLC's
//! order (`Ir::tlc_var_order`), as `TLCStateMut` does: where `compareTo`
//! would fail, or the core cannot tell that it orders as the canonical
//! order does, the core refuses. Values it cannot permute exactly, lazily
//! represented sets and functions, are refused.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::atomic::{AtomicU64, Ordering as AtomicOrdering};

use crate::Error;
use crate::handle::{Handle, Tag};
use crate::heap::Heap;
use crate::node::Kind;

/// The most images one worker's [`ImageCache`] remembers. Past it, images
/// are still computed, but no longer interned or remembered.
pub const MAX_CACHED_IMAGES: usize = 1 << 20;

/// Distinguishes reductions, so that a heap's [`ImageCache`] never serves
/// one reduction's images to another.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A worker's memo of images of persistent values: `(map index, value)` to
/// the interned image of the value under that map of one reduction.
/// Persistent values never change, so an entry stays exact for the life of
/// the store; states share most of their values with their predecessors,
/// so most images are found here instead of rebuilt.
///
/// A state variable's own value is not interned under the maps (a new
/// value per state would fill the store with images no state holds).
/// Instead, for a set variable, the sorted element images of its value in
/// the base state (the predecessor, [`Symmetry::canonical_from`]) are kept
/// per map while that base value lasts, in `roots`.
#[derive(Debug, Default)]
pub struct ImageCache {
    owner: u64,
    images: HashMap<(u32, Handle), Handle, BuildHasherDefault<KeyHasher>>,
    roots: Vec<RootImages>,
}

/// One state variable's base value and its images by map.
#[derive(Debug, Default)]
struct RootImages {
    base: Option<Handle>,
    by_map: Vec<Option<RootImage>>,
}

/// The image of a base set under one map.
#[derive(Clone, Debug)]
enum RootImage {
    /// The set itself: no element moves.
    Fixed,
    /// The images of the elements, sorted, all persistent or scalar.
    Elems(Box<[Handle]>),
}

/// A multiplicative hasher for [`ImageCache`] keys, which are words.
#[derive(Default)]
struct KeyHasher(u64);

impl Hasher for KeyHasher {
    fn finish(&self) -> u64 {
        self.0 ^ (self.0 >> 29)
    }

    fn write(&mut self, bytes: &[u8]) {
        for &b in bytes {
            self.write_u64(u64::from(b));
        }
    }

    fn write_u32(&mut self, x: u32) {
        self.write_u64(u64::from(x));
    }

    fn write_u64(&mut self, x: u64) {
        self.0 = (self.0.rotate_left(26) ^ x).wrapping_mul(0x9e37_79b9_7f4a_7c15);
    }
}

/// The closure's nonidentity maps, each as its moved model-value ids
/// `(from, to)`, sorted by `from`.
#[derive(Clone, Debug, Default)]
pub struct Symmetry {
    maps: Vec<Vec<(u32, u32)>>,
    /// This reduction's identity for [`ImageCache`] (0: none yet).
    id: u64,
    /// TLC's comparison, in this variable order (`None`: the canonical
    /// order, in declaration order).
    tlc: Option<Vec<usize>>,
}

impl Symmetry {
    /// The reduction from the maps' moved pairs of model-value ids.
    #[must_use]
    pub fn new(mut maps: Vec<Vec<(u32, u32)>>) -> Self {
        for m in &mut maps {
            m.retain(|(a, b)| a != b);
            m.sort_unstable();
        }
        Self {
            maps,
            id: NEXT_ID.fetch_add(1, AtomicOrdering::Relaxed),
            tlc: None,
        }
    }

    /// TLC's canonicalization: candidates compared with TLC's `compareTo`,
    /// variable by variable in `order` (indexes of the state's variables).
    #[must_use]
    pub fn tlc(mut self, order: Vec<usize>) -> Self {
        self.tlc = Some(order);
        self
    }

    /// The number of nonidentity maps.
    #[must_use]
    pub fn len(&self) -> usize {
        self.maps.len()
    }

    /// Whether the closure has no nonidentity map.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.maps.is_empty()
    }

    /// The canonical image of `state` into `out`: the least image under the
    /// maps, the state itself first, compared variable by variable with
    /// [`Heap::cmp`]. A later image replaces the best one only when it is
    /// strictly smaller. The images are values on `heap`, transient or
    /// persistent.
    ///
    /// # Errors
    /// A value the core cannot permute (unsupported).
    pub fn canonical(
        &self,
        heap: &mut Heap,
        state: &[Handle],
        out: &mut Vec<Handle>,
    ) -> Result<(), Error> {
        self.canonical_from(heap, state, &[], out)
    }

    /// [`Symmetry::canonical`], given `base`, a state that `state` likely
    /// shares most of its values with (its predecessor), or nothing. The
    /// result does not depend on `base`: a set variable that differs from
    /// its persistent `base` value by a few elements has its image built
    /// from the base value's remembered image (see [`ImageCache`]) instead
    /// of from all of its elements.
    ///
    /// # Errors
    /// As [`Symmetry::canonical`].
    pub fn canonical_from(
        &self,
        heap: &mut Heap,
        state: &[Handle],
        base: &[Handle],
        out: &mut Vec<Handle>,
    ) -> Result<(), Error> {
        if heap.sym_images.owner != self.id {
            heap.sym_images.owner = self.id;
            heap.sym_images.images.clear();
            heap.sym_images.roots.clear();
        }
        let mut deltas: Vec<Option<Delta>> = Vec::new();
        if base.len() == state.len() && !self.maps.is_empty() && !heap.speculative {
            if heap.sym_images.roots.len() != state.len() {
                heap.sym_images.roots.clear();
                heap.sym_images
                    .roots
                    .resize_with(state.len(), RootImages::default);
            }
            for (i, (&h, &b)) in state.iter().zip(base).enumerate() {
                deltas.push(Delta::of(heap, i, h, b));
            }
        }
        out.clear();
        out.extend_from_slice(state);
        let identity: Vec<usize>;
        let order = match &self.tlc {
            Some(order) if order.len() == state.len() => order.as_slice(),
            Some(_) => return Err(Error::internal("a SYMMETRY variable order of another size")),
            None => {
                identity = (0..state.len()).collect();
                &identity
            }
        };
        let mut cand = state.to_vec();
        let n = self.maps.len();
        for (k, map) in self.maps.iter().enumerate() {
            let k = u32::try_from(k).map_err(|_| {
                Error::unsupported("SYMMETRY with more than 2^32 permutations (core)")
            })?;
            let mut ord = Ordering::Equal;
            for &i in order {
                let (Some(&h), Some(&best)) = (state.get(i), out.get(i)) else {
                    return Err(Error::internal("a SYMMETRY variable outside the state"));
                };
                let delta = deltas.get(i).and_then(Option::as_ref);
                if ord != Ordering::Equal {
                    // Decided: the rest of the candidate is only built.
                    cand[i] = match delta {
                        Some(d) => match d.elems(heap, map, k, n)? {
                            None => h,
                            Some(list) => heap.set_sorted(&list),
                        },
                        None => permute_node(heap, map, k, h)?,
                    };
                    continue;
                }
                let elems = match delta {
                    Some(d) => Some(d.elems(heap, map, k, n)?),
                    None => None,
                };
                match elems {
                    // Compared before it is built: most images lose here.
                    Some(Some(list)) if self.tlc.is_none() && is_set(heap, best) => {
                        ord = cmp_elems(heap, &list, best);
                        cand[i] = match ord {
                            Ordering::Equal => best,
                            Ordering::Less => heap.set_sorted(&list),
                            Ordering::Greater => break,
                        };
                    }
                    elems => {
                        let img = match elems {
                            Some(None) => h,
                            Some(Some(list)) => heap.set_sorted(&list),
                            None => permute_node(heap, map, k, h)?,
                        };
                        cand[i] = img;
                        ord = if self.tlc.is_some() {
                            tlc_cmp(heap, img, best)?
                        } else {
                            heap.cmp(img, best)
                        };
                        if ord == Ordering::Greater {
                            break;
                        }
                    }
                }
            }
            if ord == Ordering::Less {
                out.clone_from(&cand);
            }
        }
        Ok(())
    }
}

/// Whether `h` is a finite set node.
fn is_set(heap: &Heap, h: Handle) -> bool {
    h.is_compound() && {
        let v = heap.view(h);
        v.kind == Kind::Set && !v.is_lazy()
    }
}

/// [`Heap::cmp`] of the set of the sorted distinct `elems` with the set
/// `best`, without building the former: sizes, then elements in order.
fn cmp_elems(heap: &Heap, elems: &[Handle], best: Handle) -> Ordering {
    let v = heap.view(best);
    elems.len().cmp(&v.data.len()).then_with(|| {
        for (&x, &y) in elems.iter().zip(v.data) {
            let c = heap.cmp(x, y);
            if c != Ordering::Equal {
                return c;
            }
        }
        Ordering::Equal
    })
}

/// The most elements a set may gain and lose from its base value for its
/// image to be built from the base value's image ([`Delta`]).
const MAX_DELTA: usize = 8;

/// A set variable's value `h` as its persistent base value less `removed`
/// plus `added`: then the image of `h` under a map is the base value's
/// image less the images of `removed` plus the images of `added`, since a
/// map is injective.
struct Delta {
    var: usize,
    base: Handle,
    added: Vec<Handle>,
    removed: Vec<Handle>,
}

impl Delta {
    /// The value `h` of variable `var` against `base`, when both are
    /// finite sets, `base` persistent, and they differ by at most
    /// [`MAX_DELTA`] elements.
    fn of(heap: &Heap, var: usize, h: Handle, base: Handle) -> Option<Self> {
        if !base.is_persistent() || !is_set(heap, h) || !is_set(heap, base) {
            return None;
        }
        let (mut added, mut removed) = (Vec::new(), Vec::new());
        if h != base {
            let (xs, ys) = (heap.view(h).data, heap.view(base).data);
            if xs.len().abs_diff(ys.len()) > MAX_DELTA {
                return None;
            }
            let (mut i, mut j) = (0, 0);
            while i < xs.len() || j < ys.len() {
                let ord = match (xs.get(i), ys.get(j)) {
                    (Some(&mine), Some(&theirs)) => heap.cmp(mine, theirs),
                    (Some(_), None) => Ordering::Less,
                    _ => Ordering::Greater,
                };
                match ord {
                    Ordering::Equal => {
                        i += 1;
                        j += 1;
                    }
                    Ordering::Less => {
                        added.push(xs[i]);
                        i += 1;
                    }
                    Ordering::Greater => {
                        removed.push(ys[j]);
                        j += 1;
                    }
                }
                if added.len() + removed.len() > MAX_DELTA {
                    return None;
                }
            }
        }
        Some(Self {
            var,
            base,
            added,
            removed,
        })
    }

    /// The sorted element images of the set this delta describes under
    /// `map` (map number `k` of `n`), or `None` when no element moves.
    fn elems(
        &self,
        heap: &mut Heap,
        map: &[(u32, u32)],
        k: u32,
        n: usize,
    ) -> Result<Option<Vec<Handle>>, Error> {
        let root = self.root(heap, map, k, n)?;
        let mut moved = matches!(root, RootImage::Elems(_));
        let mut removed = Vec::with_capacity(self.removed.len());
        for &r in &self.removed {
            let x = permute(heap, map, k, r)?;
            moved |= x != r;
            removed.push(x);
        }
        let mut added = Vec::with_capacity(self.added.len());
        for &a in &self.added {
            let x = permute(heap, map, k, a)?;
            moved |= x != a;
            added.push(x);
        }
        if !moved {
            return Ok(None);
        }
        let mut data = match root {
            RootImage::Elems(e) => e.into_vec(),
            RootImage::Fixed => heap.view(self.base).data.to_vec(),
        };
        for x in removed {
            match data.binary_search_by(|&e| heap.cmp(e, x)) {
                Ok(p) => {
                    data.remove(p);
                }
                Err(_) => {
                    return Err(Error::internal(
                        "a SYMMETRY delta removes a missing element",
                    ));
                }
            }
        }
        for x in added {
            match data.binary_search_by(|&e| heap.cmp(e, x)) {
                Ok(_) => return Err(Error::internal("a SYMMETRY delta adds a present element")),
                Err(p) => data.insert(p, x),
            }
        }
        Ok(Some(data))
    }

    /// The image of the base set under `map`, remembered for the base
    /// value while its element images are all persistent.
    fn root(
        &self,
        heap: &mut Heap,
        map: &[(u32, u32)],
        k: u32,
        n: usize,
    ) -> Result<RootImage, Error> {
        let slot = k as usize;
        if let Some(entry) = heap.sym_images.roots.get_mut(self.var) {
            if entry.base != Some(self.base) || entry.by_map.len() != n {
                entry.base = Some(self.base);
                entry.by_map.clear();
                entry.by_map.resize(n, None);
            }
            if let Some(Some(img)) = entry.by_map.get(slot) {
                return Ok(img.clone());
            }
        }
        let data = heap.view(self.base).data.to_vec();
        let mut moved = false;
        let mut keep = true;
        let mut elems = Vec::with_capacity(data.len());
        for e in data {
            let x = permute(heap, map, k, e)?;
            moved |= x != e;
            keep &= !x.is_transient();
            elems.push(x);
        }
        let img = if moved {
            // A map is injective: the images are distinct.
            elems.sort_unstable_by(|&a, &b| heap.cmp(a, b));
            RootImage::Elems(elems.into_boxed_slice())
        } else {
            RootImage::Fixed
        };
        if keep
            && let Some(cell) = heap
                .sym_images
                .roots
                .get_mut(self.var)
                .and_then(|entry| entry.by_map.get_mut(slot))
        {
            *cell = Some(img.clone());
        }
        Ok(img)
    }
}

/// TLC's `a.compareTo(b)` (`dive_value::tlc::compare`) on two finite
/// normalized values, where it orders as [`Heap::cmp`] does: values of one
/// class, and untyped model values, which TLC orders before any other
/// value. What TLC would refuse to compare (a typed model value with
/// another kind of value, values of two classes), and symbolic sets, are
/// refused.
fn tlc_cmp(heap: &Heap, a: Handle, b: Handle) -> Result<Ordering, Error> {
    use crate::compare::{Class, class, mv_type};
    if a == b {
        return Ok(Ordering::Equal);
    }
    let refuse = || {
        Error::unsupported(
            "SYMMETRY under --compat=tlc comparing values TLC may refuse to compare, or a symbolic set (core)",
        )
    };
    let (ca, cb) = (class(heap, a), class(heap, b));
    match (ca, cb) {
        (Class::Model, Class::Model) => {
            let (ta, tb) = (mv_type(heap, a), mv_type(heap, b));
            if ta.is_some() && tb.is_some() && ta != tb {
                return Err(refuse());
            }
            return Ok(heap.cmp(a, b));
        }
        (Class::Model, _) | (_, Class::Model) => {
            let model = if ca == Class::Model { a } else { b };
            if mv_type(heap, model).is_some() {
                return Err(refuse());
            }
            return Ok(heap.cmp(a, b));
        }
        _ if ca != cb => return Err(refuse()),
        (Class::Bool | Class::Int | Class::Str, _) => return Ok(heap.cmp(a, b)),
        _ => {}
    }
    let finite = |h: Handle| {
        !h.is_compound() || {
            let k = heap.view(h).kind;
            k == Kind::Set || k.is_fcn()
        }
    };
    if !finite(a) || !finite(b) {
        return Err(refuse());
    }
    if !a.is_compound() || !b.is_compound() {
        // The empty set or function: size decides.
        return Ok(heap.cmp(a, b));
    }
    let (va, vb) = (heap.view(a), heap.view(b));
    if va.kind == Kind::Set {
        let (xa, xb) = (va.data.to_vec(), vb.data.to_vec());
        if xa.len() != xb.len() {
            return Ok(xa.len().cmp(&xb.len()));
        }
        for (x, y) in xa.into_iter().zip(xb) {
            let c = tlc_cmp(heap, x, y)?;
            if c != Ordering::Equal {
                return Ok(c);
            }
        }
        return Ok(Ordering::Equal);
    }
    let n = va.len();
    if n != vb.len() {
        return Ok(n.cmp(&vb.len()));
    }
    let keys_a: Vec<Handle> = (0..n).map(|i| crate::heap::fcn_key(&va, i)).collect();
    let keys_b: Vec<Handle> = (0..n).map(|i| crate::heap::fcn_key(&vb, i)).collect();
    let (vals_a, vals_b) = (va.values().to_vec(), vb.values().to_vec());
    for (x, y) in keys_a
        .into_iter()
        .zip(keys_b)
        .chain(vals_a.into_iter().zip(vals_b))
    {
        let c = tlc_cmp(heap, x, y)?;
        if c != Ordering::Equal {
            return Ok(c);
        }
    }
    Ok(Ordering::Equal)
}

/// The image of `h`, a value inside a state variable's value, under `map`
/// (the reduction's map number `k`); `h` itself when nothing in it moves.
/// The image of a persistent value is interned and remembered in the
/// heap's [`ImageCache`], outside speculative candidate expansion, which
/// must not promote values.
fn permute(heap: &mut Heap, map: &[(u32, u32)], k: u32, h: Handle) -> Result<Handle, Error> {
    if !h.is_compound() || !h.is_persistent() || heap.speculative {
        return permute_node(heap, map, k, h);
    }
    if let Some(&img) = heap.sym_images.images.get(&(k, h)) {
        return Ok(img);
    }
    let img = permute_node(heap, map, k, h)?;
    if heap.sym_images.images.len() >= MAX_CACHED_IMAGES {
        return Ok(img);
    }
    let img = if img == h { h } else { heap.intern(img)? };
    heap.sym_images.images.insert((k, h), img);
    Ok(img)
}

/// [`permute`] on one node, unremembered: its children through
/// [`permute`]. A state variable's value is permuted with this.
#[allow(
    clippy::many_single_char_names,
    reason = "short names for a value, its view and its entries"
)]
fn permute_node(heap: &mut Heap, map: &[(u32, u32)], k: u32, h: Handle) -> Result<Handle, Error> {
    match h.tag() {
        Tag::Model => {
            let Some(id) = h.as_model_id() else {
                return Ok(h);
            };
            return Ok(match map.binary_search_by_key(&id, |&(a, _)| a) {
                Ok(i) => Handle::model(map[i].1),
                Err(_) => h,
            });
        }
        Tag::Bool | Tag::Int | Tag::Str => return Ok(h),
        Tag::Persistent | Tag::Transient | Tag::Empty => {}
    }
    if !h.is_compound() {
        return Ok(h);
    }
    let v = heap.view(h);
    let kind = v.kind;
    let lo = v.lo;
    let data = v.data.to_vec();
    match kind {
        Kind::BigInt | Kind::Interval | Kind::Nat | Kind::Int => Ok(h),
        Kind::Set => {
            let mut moved = false;
            let mut elems = Vec::with_capacity(data.len());
            for e in data {
                let x = permute(heap, map, k, e)?;
                moved |= x != e;
                elems.push(x);
            }
            Ok(if moved { heap.set_from(&mut elems) } else { h })
        }
        Kind::IntFcn => {
            let mut moved = false;
            let mut vals = Vec::with_capacity(data.len());
            for e in data {
                let x = permute(heap, map, k, e)?;
                moved |= x != e;
                vals.push(x);
            }
            Ok(if moved { heap.int_fcn(lo, &vals) } else { h })
        }
        Kind::Fcn => {
            let n = data.len() / 2;
            let mut moved = false;
            let mut pairs = Vec::with_capacity(n);
            for i in 0..n {
                let (key, e) = (data[i], data[n + i]);
                let (pk, pe) = (permute(heap, map, k, key)?, permute(heap, map, k, e)?);
                moved |= pk != key || pe != e;
                pairs.push((pk, pe));
            }
            Ok(if moved {
                heap.fcn_from_pairs(&mut pairs)
            } else {
                h
            })
        }
        _ => Err(Error::unsupported(
            "SYMMETRY over a state holding a lazily represented set or function (core)",
        )),
    }
}

#[cfg(test)]
mod tests {
    #![allow(
        clippy::unwrap_used,
        clippy::many_single_char_names,
        reason = "tests over short value names"
    )]
    use std::sync::Arc;

    use super::*;
    use crate::store::Store;

    #[test]
    fn the_canonical_image_is_the_least_permuted_state() {
        let mut h = Heap::new(Arc::new(Store::default()), false, false);
        let mv = |h: &Heap, t: &str| Handle::model(h.store().strings().intern(t).unwrap());
        let (a, b) = (mv(&h, "a"), mv(&h, "b"));
        let one = Handle::int(1).unwrap();
        let sym = Symmetry::new(vec![vec![
            (a.as_model_id().unwrap(), b.as_model_id().unwrap()),
            (b.as_model_id().unwrap(), a.as_model_id().unwrap()),
        ]]);
        // [a |-> 1, b |-> 2] and [a |-> 2, b |-> 1] share a canonical image.
        let two = Handle::int(2).unwrap();
        let f = h.fcn_from_pairs(&mut [(a, one), (b, two)]);
        let g = h.fcn_from_pairs(&mut [(a, two), (b, one)]);
        let (mut x, mut y) = (Vec::new(), Vec::new());
        sym.canonical(&mut h, &[f, b], &mut x).unwrap();
        sym.canonical(&mut h, &[g, a], &mut y).unwrap();
        assert!(h.eq(x[0], y[0]) && h.eq(x[1], y[1]));
        // A state without model values is its own image.
        let s = h.set_from(&mut vec![one, two]);
        sym.canonical(&mut h, &[s], &mut x).unwrap();
        assert_eq!(x, vec![s]);
    }

    #[test]
    fn images_from_a_base_state_and_its_memo_equal_images_from_scratch() {
        let mut h = Heap::new(Arc::new(Store::default()), false, false);
        let mv = |h: &Heap, t: &str| Handle::model(h.store().strings().intern(t).unwrap());
        let vals = [mv(&h, "a"), mv(&h, "b"), mv(&h, "c")];
        let ids: Vec<u32> = vals.iter().map(|v| v.as_model_id().unwrap()).collect();
        // Every nonidentity permutation of {a, b, c}.
        let perms = [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
        let maps = perms
            .iter()
            .map(|p| (0..3).map(|i| (ids[i], ids[p[i]])).collect())
            .collect();
        let sym = Symmetry::new(maps);
        let int = |n: i64| Handle::int(n).unwrap();
        // Messages <<v, n>>, as a set variable, and a function variable.
        let msg = |h: &mut Heap, v: usize, n: i64| {
            let t = h.tuple(&[vals[v], int(n)]);
            h.intern(t).unwrap()
        };
        let all: Vec<Handle> = (0..3)
            .flat_map(|v| (1..4).map(move |n| (v, n)))
            .map(|(v, n)| msg(&mut h, v, n))
            .collect();
        let pick = |h: &mut Heap, idx: &[usize]| {
            let mut e: Vec<Handle> = idx.iter().map(|&i| all[i]).collect();
            h.set_from(&mut e)
        };
        let cases: [(&[usize], &[usize]); 5] = [
            (&[0, 4, 8], &[0, 4]),
            (&[0, 4], &[0, 4]),
            (&[1, 2, 6], &[1, 2, 3, 6]),
            (&[5], &[0, 1, 2, 3, 4, 5, 6, 7, 8]),
            (&[2, 3, 7, 8], &[1, 3, 7]),
        ];
        for (state_elems, base_elems) in cases {
            for round in 0..2 {
                let s = pick(&mut h, state_elems);
                let b = pick(&mut h, base_elems);
                let b = h.intern(b).unwrap();
                let f = h.fcn_from_pairs(&mut [
                    (vals[0], int(1)),
                    (vals[1], int(2)),
                    (vals[2], int(2)),
                ]);
                // The state's set is the base value itself when they agree.
                let s = if state_elems == base_elems { b } else { s };
                let (mut x, mut y) = (Vec::new(), Vec::new());
                // From scratch: speculative expansion uses no memo.
                h.set_speculative(true);
                sym.canonical(&mut h, &[s, f], &mut x).unwrap();
                h.set_speculative(false);
                sym.canonical_from(&mut h, &[s, f], &[b, f], &mut y)
                    .unwrap();
                assert!(
                    x.len() == y.len() && x.iter().zip(&y).all(|(&p, &q)| h.eq(p, q)),
                    "{state_elems:?} from {base_elems:?}, round {round}"
                );
            }
        }
    }

    #[test]
    fn tlc_comparison_follows_its_variable_order_and_refuses_what_tlc_fails_on() {
        let mut h = Heap::new(Arc::new(Store::default()), false, false);
        let mv = |h: &Heap, t: &str| Handle::model(h.store().strings().intern(t).unwrap());
        let (a, b) = (mv(&h, "a"), mv(&h, "b"));
        let swap = vec![
            (a.as_model_id().unwrap(), b.as_model_id().unwrap()),
            (b.as_model_id().unwrap(), a.as_model_id().unwrap()),
        ];
        // <<a, b>> and <<b, a>>: in declaration order the image starts
        // with `a`; with the second variable first, the second is `a`.
        let mut out = Vec::new();
        Symmetry::new(vec![swap.clone()])
            .canonical(&mut h, &[b, a], &mut out)
            .unwrap();
        assert_eq!(out, vec![a, b]);
        Symmetry::new(vec![swap.clone()])
            .tlc(vec![1, 0])
            .canonical(&mut h, &[a, b], &mut out)
            .unwrap();
        assert_eq!(out, vec![b, a]);
        // An untyped model value comes before any other value.
        let one = Handle::int(1).unwrap();
        assert_eq!(tlc_cmp(&h, a, one).unwrap(), Ordering::Less);
        assert_eq!(tlc_cmp(&h, one, a).unwrap(), Ordering::Greater);
        // TLC fails on a typed model value against an integer, and on two
        // classes.
        let typed = mv(&h, "A_x");
        assert!(tlc_cmp(&h, typed, one).is_err());
        let s = h.set_from(&mut vec![one]);
        assert!(tlc_cmp(&h, s, one).is_err());
    }
}
