//! Symmetry reduction (`SYMMETRY`): the state key of a state is the key of
//! its canonical image, the least of its images under the closure's
//! permutations of model values, compared variable by variable in the
//! run's value order. The explored state stays the concrete one; only keys
//! change, as in `dive-model`'s symmetry reduction
//! (`Reduction::canonical_values`).
//!
//! The default profile's canonicalization compares in the canonical
//! value order, variables in declaration order. TLC's (`--compat=tlc`,
//! [`Symmetry::tlc`]) compares with TLC's `compareTo`, variables in TLC's
//! order (`Ir::tlc_var_order`), as `TLCStateMut` does: where `compareTo`
//! would fail, or the core cannot tell that it orders as the canonical
//! order does, the core refuses. Values it cannot permute exactly, lazily
//! represented sets and functions, are refused.

use std::cmp::Ordering;
use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering as AtomicOrdering};

use crate::Error;
use crate::handle::{Handle, Tag};
use crate::heap::Heap;
use crate::node::{Kind, flags};

/// The most images one worker's [`ImageCache`] remembers. Past it, images
/// are still computed, but no longer interned or remembered.
pub const MAX_CACHED_IMAGES: usize = 1 << 20;

/// The most bytes one reduction's remembered images may take, across all
/// workers: the interned images in the shared store and the memo entries.
/// Past it, images are still computed, but no longer interned or
/// remembered; the images already remembered still serve.
pub const MAX_IMAGE_BYTES: usize = 32 << 20;

/// The most values one worker's [`ImageCache`] tracks as seen once. When it
/// is full it is cleared, which only delays admitting values.
const MAX_SEEN: usize = 1 << 18;

/// The images a worker's [`ImageCache`] remembers before it must have
/// served [`MIN_HITS`] lookups per image it holds.
const WARM_UP: usize = 1 << 10;

/// The lookups a worker's [`ImageCache`] must have served per image it
/// holds, past [`WARM_UP`], to remember another.
const MIN_HITS: u64 = 4;

/// The bytes charged for one memo entry beside its image.
const ENTRY_BYTES: usize = 32;

/// Distinguishes reductions, so that a heap's [`ImageCache`] never serves
/// one reduction's images to another.
static NEXT_ID: AtomicU64 = AtomicU64::new(1);

/// A worker's memo of images of persistent values: `(map index, value)` to
/// the interned image of the value under that map of one reduction.
/// Persistent values never change, so an entry stays exact for the life of
/// the store; states share most of their values with their predecessors,
/// so most images are found here instead of rebuilt.
///
/// A value's images are interned only once the value is seen again in a
/// later canonicalization (`seen`): a value inside one state only, as in a
/// nested function rewritten by every step, is permuted transiently and
/// costs the store nothing. The bytes remembered are bounded per reduction
/// across workers by [`MAX_IMAGE_BYTES`].
///
/// A persistent state variable's own value, such as one a step left
/// unchanged, is remembered under the same admission rules, so a value
/// that recurs is permuted once per map. For a set variable with a delta
/// from its value in the base state (the predecessor,
/// [`Symmetry::canonical_from`]), the sorted element images of the base
/// value are kept per map while that base value lasts, in `roots`.
///
/// The images of function domains are kept in `domains`
/// (`DomainImage`), so that a function's image is a gather of its
/// permuted values, without sorting its keys.
#[derive(Debug, Default)]
pub struct ImageCache {
    owner: u64,
    images: HashMap<(u32, Handle), Handle, BuildHasherDefault<KeyHasher>>,
    /// Persistent values whose images are not remembered, with the number
    /// of the canonicalization that first met them.
    seen: HashMap<Handle, u64, BuildHasherDefault<KeyHasher>>,
    /// The number of the current canonicalization.
    epoch: u64,
    /// The memo's lookups that found an image.
    hits: u64,
    roots: Vec<RootImages>,
    /// Images of function domains, by map index, then the domain's keys
    /// ([`DomainImage`]).
    domains: Vec<DomainMap>,
    /// The keys `domains` holds, bounded by [`MAX_DOMAIN_KEYS`].
    domain_keys: usize,
    /// Buffers for [`cmp_image`], kept between calls.
    scratch: (Vec<Handle>, Vec<Handle>),
    /// Emptied after each canonicalization. Only their capacities cross a state boundary.
    rows: (Vec<Option<Handle>>, Vec<Option<Handle>>),
    deltas: Vec<Option<Delta>>,
}

/// One map's [`DomainImage`]s, keyed by the sorted keys of the domain.
type DomainMap = HashMap<Box<[Handle]>, Arc<DomainImage>, BuildHasherDefault<KeyHasher>>;

/// The most keys, over all domains, one worker's [`ImageCache`] remembers
/// [`DomainImage`]s for. Past it, domain images are still computed, but no
/// longer remembered; those already remembered still serve.
const MAX_DOMAIN_KEYS: usize = 1 << 20;

/// The image of the sorted keys of a function's domain under one map: the
/// image keys, sorted, and for each the position of the key it is the image
/// of. A function on that domain has its image by gather: the image's value
/// at position `j` is the image of the value at position `src[j]`, since
/// the image of `f` maps `π(x)` to `π(f[x])`. No key is compared or sorted.
#[derive(Debug)]
struct DomainImage {
    /// The image keys, sorted.
    keys: Box<[Handle]>,
    /// For each image key, the position of its preimage among the keys.
    src: Box<[u32]>,
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

    fn write_usize(&mut self, x: usize) {
        self.write_u64(x as u64);
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
    /// The bytes this reduction's [`ImageCache`]s hold, across workers.
    spent: Arc<AtomicUsize>,
    /// The bound on `spent` ([`MAX_IMAGE_BYTES`]).
    limit: usize,
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
            spent: Arc::new(AtomicUsize::new(0)),
            limit: MAX_IMAGE_BYTES,
            tlc: None,
        }
    }

    /// This reduction with at most `limit` bytes of remembered images.
    #[cfg(test)]
    fn with_image_limit(mut self, limit: usize) -> Self {
        self.limit = limit;
        self
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
            heap.sym_images.seen.clear();
            heap.sym_images.roots.clear();
            heap.sym_images.domains.clear();
            heap.sym_images.domain_keys = 0;
        }
        heap.sym_images.epoch += 1;
        let mut deltas = std::mem::take(&mut heap.sym_images.deltas);
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
        let order = self.tlc.as_deref();
        if order.is_some_and(|order| order.len() != state.len()) {
            return Err(Error::internal("a SYMMETRY variable order of another size"));
        }
        // The best image so far is the state under map `best_map` (`None`:
        // the state itself), built lazily: `best[i]` is its value of
        // variable `i` once built. A candidate is built only up to the
        // variable that decides it, and a winner's other variables only
        // when a later candidate is compared with them, or at the end.
        let mut best_map: Option<u32> = None;
        let (mut best, mut cand) = std::mem::take(&mut heap.sym_images.rows);
        best.extend(state.iter().copied().map(Some));
        cand.resize(state.len(), None);
        for (k, map) in self.maps.iter().enumerate() {
            let k = u32::try_from(k).map_err(|_| {
                Error::unsupported("SYMMETRY with more than 2^32 permutations (core)")
            })?;
            cand.fill(None);
            let mut ord = Ordering::Equal;
            for position in 0..state.len() {
                let i = order.map_or(position, |order| order[position]);
                let Some(&h) = state.get(i) else {
                    return Err(Error::internal("a SYMMETRY variable outside the state"));
                };
                let best_i = if let Some(b) = best.get(i).copied().flatten() {
                    b
                } else {
                    let b = self.image_of(heap, &deltas, best_map, i, h)?;
                    best[i] = Some(b);
                    b
                };
                let delta = deltas.get(i).and_then(Option::as_ref);
                let (c, img) = self.cmp_var(heap, delta, map, k, h, best_i)?;
                let Some(img) = img else {
                    break;
                };
                cand[i] = Some(img);
                if c != Ordering::Equal {
                    ord = c;
                    break;
                }
            }
            if ord == Ordering::Less {
                best_map = Some(k);
                std::mem::swap(&mut best, &mut cand);
            }
        }
        out.clear();
        for (i, &h) in state.iter().enumerate() {
            let img = match best.get(i).copied().flatten() {
                Some(b) => b,
                None => self.image_of(heap, &deltas, best_map, i, h)?,
            };
            out.push(img);
        }
        best.clear();
        cand.clear();
        deltas.clear();
        heap.sym_images.rows = (best, cand);
        heap.sym_images.deltas = deltas;
        Ok(())
    }

    /// The image of `value`, the state's variable `var`, under map number
    /// `at` (`None`: `value` itself), from its delta when it has one.
    fn image_of(
        &self,
        heap: &mut Heap,
        deltas: &[Option<Delta>],
        at: Option<u32>,
        var: usize,
        value: Handle,
    ) -> Result<Handle, Error> {
        let Some(index) = at else {
            return Ok(value);
        };
        let Some(map) = self.maps.get(index as usize) else {
            return Err(Error::internal("a SYMMETRY map outside the closure"));
        };
        Ok(match deltas.get(var).and_then(Option::as_ref) {
            Some(delta) => match delta.elems(self, heap, map, index, self.maps.len())? {
                None => value,
                Some(list) => heap.set_sorted(&list),
            },
            None => permute(self, heap, map, index, value)?,
        })
    }

    /// Compares the image of a variable's value `h` under `map` (map
    /// number `k`) with `best`, the best image's value of the variable:
    /// the order and the image, which is `best` when they are equal and is
    /// not built (`None`) when it is greater, where possible.
    #[allow(
        clippy::many_single_char_names,
        reason = "short names for a value and its map"
    )]
    fn cmp_var(
        &self,
        heap: &mut Heap,
        delta: Option<&Delta>,
        map: &[(u32, u32)],
        k: u32,
        h: Handle,
        best: Handle,
    ) -> Result<(Ordering, Option<Handle>), Error> {
        let elems = match delta {
            Some(d) => Some(d.elems(self, heap, map, k, self.maps.len())?),
            None => None,
        };
        Ok(match elems {
            // Compared before it is built: most images lose here.
            Some(Some(list)) if self.tlc.is_none() && is_set(heap, best) => {
                match cmp_elems(heap, &list, best) {
                    Ordering::Equal => (Ordering::Equal, Some(best)),
                    Ordering::Less => (Ordering::Less, Some(heap.set_sorted(&list))),
                    Ordering::Greater => (Ordering::Greater, None),
                }
            }
            // A remembered image is compared as it is.
            None if h.is_persistent()
                && !heap.speculative
                && let Some(&img) = heap.sym_images.images.get(&(k, h)) =>
            {
                heap.sym_images.hits += 1;
                let c = if self.tlc.is_some() {
                    tlc_cmp(heap, img, best)?
                } else {
                    heap.cmp(img, best)
                };
                (c, Some(img))
            }
            None if self.tlc.is_none()
                && let Some(c) = cmp_image(self, heap, map, k, h, best)? =>
            {
                match c {
                    Ordering::Equal => (Ordering::Equal, Some(best)),
                    Ordering::Less => (Ordering::Less, Some(permute(self, heap, map, k, h)?)),
                    Ordering::Greater => (Ordering::Greater, None),
                }
            }
            elems => {
                let img = match elems {
                    Some(None) => h,
                    Some(Some(list)) => heap.set_sorted(&list),
                    None => permute(self, heap, map, k, h)?,
                };
                let c = if self.tlc.is_some() {
                    tlc_cmp(heap, img, best)?
                } else {
                    heap.cmp(img, best)
                };
                (c, Some(img))
            }
        })
    }
}

/// Whether `h` is an eager function node (`IntFcn` or `Fcn`) whose
/// values are all closed.
fn is_fcn_node(heap: &Heap, h: Handle) -> bool {
    h.is_compound() && {
        let v = heap.view(h);
        matches!(v.kind, Kind::IntFcn | Kind::Fcn) && !v.is_lazy()
    }
}

/// [`Heap::cmp`] of the image of `h` under `map` (map number `k`) with
/// `best`, without building the image, when both are eager functions and
/// `h` is proved materialized ([`flags::CONCRETE`]), so that permuting any
/// part of it cannot fail; otherwise `None`. Functions compare by size,
/// then keys in order, then values in key order: the image's keys come
/// from its [`DomainImage`], and its values are permuted one at a time,
/// up to the first that differs. Most images lose at their first values.
#[allow(
    clippy::many_single_char_names,
    reason = "short names for a value, its view and its entries"
)]
fn cmp_image(
    sym: &Symmetry,
    heap: &mut Heap,
    map: &[(u32, u32)],
    k: u32,
    h: Handle,
    best: Handle,
) -> Result<Option<Ordering>, Error> {
    if !is_fcn_node(heap, h) || !is_fcn_node(heap, best) {
        return Ok(None);
    }
    let v = heap.view(h);
    if v.flags & flags::CONCRETE == 0 {
        return Ok(None);
    }
    let n = v.len();
    let bv = heap.view(best);
    if n != bv.len() {
        return Ok(Some(n.cmp(&bv.len())));
    }
    let (kind, lo) = (v.kind, v.lo);
    let (best_kind, best_lo) = (bv.kind, bv.lo);
    // Copies of the entries, in buffers kept between calls.
    let (mut data, mut best_data) = std::mem::take(&mut heap.sym_images.scratch);
    data.clear();
    data.extend_from_slice(heap.view(h).data);
    best_data.clear();
    best_data.extend_from_slice(heap.view(best).data);
    let r = cmp_entries(
        sym,
        heap,
        (map, k),
        (kind, lo, &data),
        (best_kind, best_lo, &best_data),
    );
    heap.sym_images.scratch = (data, best_data);
    r.map(Some)
}

/// [`cmp_image`] on the entries of the function and of `best`, each with
/// its kind and lower bound.
fn cmp_entries(
    sym: &Symmetry,
    heap: &mut Heap,
    (map, k): (&[(u32, u32)], u32),
    (kind, lo, data): (Kind, i64, &[Handle]),
    (best_kind, best_lo, best_data): (Kind, i64, &[Handle]),
) -> Result<Ordering, Error> {
    let n = match kind {
        Kind::Fcn => data.len() / 2,
        _ => data.len(),
    };
    let best_vals = match best_kind {
        Kind::Fcn => &best_data[n..],
        _ => best_data,
    };
    let (vals, dom) = match kind {
        Kind::Fcn => (&data[n..], domain_image(sym, heap, map, k, &data[..n])?),
        _ => (data, None),
    };
    // The keys, unless both are functions over the same integers.
    if kind != Kind::IntFcn || best_kind != Kind::IntFcn || lo != best_lo {
        for i in 0..n {
            let mine = match (&dom, kind) {
                (Some(d), _) => d.keys.get(i).copied(),
                (None, Kind::Fcn) => data.get(i).copied(),
                #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
                (None, _) => Some(Handle::int(lo + i as i64).unwrap_or(Handle::EMPTY)),
            };
            #[allow(clippy::cast_possible_wrap, reason = "domains fit in i64")]
            let theirs = match best_kind {
                Kind::Fcn => best_data.get(i).copied(),
                _ => Some(Handle::int(best_lo + i as i64).unwrap_or(Handle::EMPTY)),
            };
            let (Some(mine), Some(theirs)) = (mine, theirs) else {
                return Err(Error::internal("a SYMMETRY function image of another size"));
            };
            let c = heap.cmp(mine, theirs);
            if c != Ordering::Equal {
                return Ok(c);
            }
        }
    }
    for (j, &theirs) in best_vals.iter().enumerate() {
        let at = dom
            .as_ref()
            .map_or(Some(j), |d| d.src.get(j).map(|&i| i as usize));
        let Some(&e) = at.and_then(|i| vals.get(i)) else {
            return Err(Error::internal("a SYMMETRY function image of another size"));
        };
        let mine = permute(sym, heap, map, k, e)?;
        let c = heap.cmp(mine, theirs);
        if c != Ordering::Equal {
            return Ok(c);
        }
    }
    Ok(Ordering::Equal)
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
#[derive(Debug)]
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
        sym: &Symmetry,
        heap: &mut Heap,
        map: &[(u32, u32)],
        k: u32,
        n: usize,
    ) -> Result<Option<Vec<Handle>>, Error> {
        let root = self.root(sym, heap, map, k, n)?;
        let mut moved = matches!(root, RootImage::Elems(_));
        let mut removed = Vec::with_capacity(self.removed.len());
        for &r in &self.removed {
            let x = permute(sym, heap, map, k, r)?;
            moved |= x != r;
            removed.push(x);
        }
        let mut added = Vec::with_capacity(self.added.len());
        for &a in &self.added {
            let x = permute(sym, heap, map, k, a)?;
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
        sym: &Symmetry,
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
            let x = permute(sym, heap, map, k, e)?;
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
        let (xa, xb) = (va.data, vb.data);
        if xa.len() != xb.len() {
            return Ok(xa.len().cmp(&xb.len()));
        }
        for (&x, &y) in xa.iter().zip(xb) {
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
    let keys = (0..n).map(|i| (crate::heap::fcn_key(&va, i), crate::heap::fcn_key(&vb, i)));
    let values = va.values().iter().copied().zip(vb.values().iter().copied());
    for (x, y) in keys.chain(values) {
        let c = tlc_cmp(heap, x, y)?;
        if c != Ordering::Equal {
            return Ok(c);
        }
    }
    Ok(Ordering::Equal)
}

/// The image of `h`, a state variable's value or a value inside one, under `map`
/// (the reduction's map number `k`); `h` itself when nothing in it moves.
/// The image of a persistent value is interned and remembered in the
/// heap's [`ImageCache`], outside speculative candidate expansion, which
/// must not promote values.
#[inline]
fn permute(
    sym: &Symmetry,
    heap: &mut Heap,
    map: &[(u32, u32)],
    k: u32,
    h: Handle,
) -> Result<Handle, Error> {
    if !h.is_compound() || !h.is_persistent() || heap.speculative {
        return permute_node(sym, heap, map, k, h);
    }
    permute_remembered(sym, heap, map, k, h)
}

/// [`permute`] on a persistent compound value, through the memo.
#[inline(never)]
fn permute_remembered(
    sym: &Symmetry,
    heap: &mut Heap,
    map: &[(u32, u32)],
    k: u32,
    h: Handle,
) -> Result<Handle, Error> {
    if let Some(&img) = heap.sym_images.images.get(&(k, h)) {
        heap.sym_images.hits += 1;
        return Ok(img);
    }
    let img = permute_node(sym, heap, map, k, h)?;
    let cache = &mut heap.sym_images;
    // Admitted when met again in a later canonicalization: a value of one
    // state only is permuted transiently, at no cost to the store.
    let epoch = cache.epoch;
    match cache.seen.get(&h) {
        Some(&first) if first != epoch => {}
        Some(_) => return Ok(img),
        None => {
            if cache.seen.len() >= MAX_SEEN {
                cache.seen.clear();
            }
            cache.seen.insert(h, epoch);
            return Ok(img);
        }
    }
    // Past a warm-up, an image is remembered only while the memo has
    // served [`MIN_HITS`] lookups for each image it holds: an image used
    // about once costs more to intern than to rebuild.
    let held = cache.images.len();
    if held >= MAX_CACHED_IMAGES
        || (held >= WARM_UP && cache.hits < MIN_HITS * held as u64)
        || sym.spent.load(AtomicOrdering::Relaxed) >= sym.limit
    {
        return Ok(img);
    }
    let cost = ENTRY_BYTES + transient_bytes(heap, img);
    // Workers may each overshoot the limit by one image.
    sym.spent.fetch_add(cost, AtomicOrdering::Relaxed);
    let img = if img == h { h } else { heap.intern(img)? };
    heap.sym_images.images.insert((k, h), img);
    Ok(img)
}

/// The image of the sorted, distinct `keys` of a function's domain under
/// `map` (map number `k`), or `None` when no key moves. It is remembered
/// for the keys when they and their images are all persistent or scalar.
fn domain_image(
    sym: &Symmetry,
    heap: &mut Heap,
    map: &[(u32, u32)],
    k: u32,
    keys: &[Handle],
) -> Result<Option<Arc<DomainImage>>, Error> {
    // Strings, integers and Booleans never move (a record's fields).
    if keys
        .iter()
        .all(|&x| matches!(x.tag(), Tag::Str | Tag::Int | Tag::Bool))
    {
        return Ok(None);
    }
    let slot = k as usize;
    let lasting = keys.iter().all(|&x| !x.is_transient());
    if lasting && let Some(dom) = heap.sym_images.domains.get(slot).and_then(|m| m.get(keys)) {
        // Empty image keys: no key moves.
        return Ok((!dom.keys.is_empty()).then(|| dom.clone()));
    }
    let mut moved = false;
    let mut images = Vec::with_capacity(keys.len());
    for (i, &x) in keys.iter().enumerate() {
        let y = permute(sym, heap, map, k, x)?;
        moved |= y != x;
        let i = u32::try_from(i)
            .map_err(|_| Error::unsupported("SYMMETRY over a function of 2^32 entries (core)"))?;
        images.push((y, i));
    }
    let dom = if moved {
        // A map is injective: the image keys are distinct.
        images.sort_unstable_by(|a, b| heap.cmp(a.0, b.0));
        Arc::new(DomainImage {
            keys: images.iter().map(|p| p.0).collect(),
            src: images.iter().map(|p| p.1).collect(),
        })
    } else {
        Arc::new(DomainImage {
            keys: Box::default(),
            src: Box::default(),
        })
    };
    let cache = &mut heap.sym_images;
    if lasting
        && dom.keys.iter().all(|&x| !x.is_transient())
        && cache.domain_keys + keys.len() <= MAX_DOMAIN_KEYS
    {
        if cache.domains.len() <= slot {
            cache.domains.resize_with(slot + 1, DomainMap::default);
        }
        if let Some(m) = cache.domains.get_mut(slot) {
            cache.domain_keys += keys.len();
            m.insert(keys.into(), dom.clone());
        }
    }
    Ok(moved.then_some(dom))
}

/// The bytes the transient nodes of `h` take, which interning copies into
/// the store.
fn transient_bytes(heap: &Heap, h: Handle) -> usize {
    if !h.is_compound() || !h.is_transient() {
        return 0;
    }
    let data = heap.view(h).data;
    let own = 16 + std::mem::size_of_val(data);
    data.iter().fold(own, |n, &c| n + transient_bytes(heap, c))
}

/// [`permute`] on one node, unremembered: its children through
/// [`permute`]. A function is permuted through its [`DomainImage`].
#[allow(
    clippy::many_single_char_names,
    reason = "short names for a value, its view and its entries"
)]
fn permute_node(
    sym: &Symmetry,
    heap: &mut Heap,
    map: &[(u32, u32)],
    k: u32,
    h: Handle,
) -> Result<Handle, Error> {
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
    let view_bits = v.flags;
    let data = v.data.to_vec();
    match kind {
        Kind::BigInt | Kind::Interval | Kind::Nat | Kind::Int => Ok(h),
        Kind::Set => {
            let mut moved = false;
            let mut elems = Vec::with_capacity(data.len());
            for e in data {
                let x = permute(sym, heap, map, k, e)?;
                moved |= x != e;
                elems.push(x);
            }
            Ok(if moved { heap.set_from(&mut elems) } else { h })
        }
        Kind::IntFcn => {
            let mut moved = false;
            let mut vals = Vec::with_capacity(data.len());
            for e in data {
                let x = permute(sym, heap, map, k, e)?;
                moved |= x != e;
                vals.push(x);
            }
            // The keys do not move: the image keeps the representation.
            Ok(if moved {
                heap.int_fcn_as(lo, &vals, view_bits & flags::TUPLE)
            } else {
                h
            })
        }
        Kind::Fcn => {
            let n = data.len() / 2;
            let (keys, vals) = data.split_at(n);
            let mut image = Vec::with_capacity(n);
            match domain_image(sym, heap, map, k, keys)? {
                None => {
                    // No key moves: the image keeps the key order and the
                    // representation (a record stays a record).
                    let mut moved = false;
                    for &e in vals {
                        let x = permute(sym, heap, map, k, e)?;
                        moved |= x != e;
                        image.push(x);
                    }
                    Ok(if moved {
                        heap.fcn_sorted_as(keys, &image, view_bits & flags::RECORD)
                    } else {
                        h
                    })
                }
                Some(dom) => {
                    for &i in &dom.src {
                        let Some(&e) = vals.get(i as usize) else {
                            return Err(Error::internal("a SYMMETRY domain image of another size"));
                        };
                        image.push(permute(sym, heap, map, k, e)?);
                    }
                    // A record's keys are strings, which never move.
                    Ok(heap.fcn_sorted(&dom.keys, &image))
                }
            }
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

    /// The reduction of every permutation of three model values, the
    /// values, and `n` distinct interned functions over them, each held in
    /// a transient state value `[1 |-> f]`.
    fn nested_states(n: usize) -> (Heap, Symmetry, Vec<Handle>) {
        let mut h = Heap::new(Arc::new(Store::default()), false, false);
        let mv = |h: &Heap, t: &str| Handle::model(h.store().strings().intern(t).unwrap());
        let vals = [mv(&h, "a"), mv(&h, "b"), mv(&h, "c")];
        let ids: Vec<u32> = vals.iter().map(|v| v.as_model_id().unwrap()).collect();
        let perms = [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
        let maps = perms
            .iter()
            .map(|p| (0..3).map(|i| (ids[i], ids[p[i]])).collect())
            .collect();
        let int = |n: i64| Handle::int(n).unwrap();
        let inner: Vec<Handle> = (0..n)
            .map(|i| {
                // Position j holds base-3 digit j of i.
                let mut pairs: Vec<(Handle, Handle)> = (0..8u32)
                    .map(|j| (int(i64::from(j)), vals[i / 3usize.pow(j) % 3]))
                    .collect();
                let f = h.fcn_from_pairs(&mut pairs);
                h.intern(f).unwrap()
            })
            .collect();
        (h, Symmetry::new(maps), inner)
    }

    /// The canonical image of `[1 |-> f]`, checked against the image
    /// computed without the memo.
    fn canonical_nested(h: &mut Heap, sym: &Symmetry, f: Handle) {
        let one = Handle::int(1).unwrap();
        let v = h.fcn_from_pairs(&mut [(one, f)]);
        let (mut x, mut y) = (Vec::new(), Vec::new());
        h.set_speculative(true);
        sym.canonical(h, &[v], &mut x).unwrap();
        h.set_speculative(false);
        sym.canonical(h, &[v], &mut y).unwrap();
        assert!(h.eq(x[0], y[0]));
    }

    #[test]
    fn values_met_in_one_canonicalization_add_nothing_to_the_store() {
        // A nested value that each state rewrites (#876 review): its images
        // are never wanted again, so they must stay transient.
        let (mut h, sym, inner) = nested_states(300);
        let before = h.store().stats().nodes;
        for &f in &inner {
            canonical_nested(&mut h, &sym, f);
        }
        assert_eq!(h.store().stats().nodes, before);
        assert!(h.sym_images.images.is_empty());
        assert_eq!(sym.spent.load(AtomicOrdering::Relaxed), 0);
    }

    #[test]
    fn images_that_serve_no_later_lookup_stop_being_remembered() {
        // Each value is met twice and then never again: past the warm-up,
        // the memo has served too few lookups to remember more.
        let (mut h, sym, inner) = nested_states(600);
        for &f in &inner {
            canonical_nested(&mut h, &sym, f);
            canonical_nested(&mut h, &sym, f);
        }
        let held = h.sym_images.images.len();
        assert!(held >= WARM_UP && held <= WARM_UP + sym.len(), "{held}");
    }

    #[test]
    fn remembered_images_stay_within_the_reductions_byte_limit() {
        let (mut h, sym, inner) = nested_states(200);
        let limit = 4096;
        let sym = sym.with_image_limit(limit);
        // Every value recurs, so every lookup after the first two hits.
        for _ in 0..4 {
            for &f in &inner {
                canonical_nested(&mut h, &sym, f);
            }
        }
        let spent = sym.spent.load(AtomicOrdering::Relaxed);
        assert!(spent >= limit, "{spent}");
        // At most one image past the limit: [j \in 0..7 |-> v] and its entry.
        assert!(spent < limit + 512, "{spent}");
        assert!(h.sym_images.images.len() < limit / ENTRY_BYTES);
    }

    /// The image of `h` under `map`, built by sorting, as permuting did
    /// before domain images: the reference for [`permute_node`].
    fn sorted_image(h: &mut Heap, map: &[(u32, u32)], x: Handle) -> Handle {
        if let Some(id) = x.as_model_id() {
            return match map.binary_search_by_key(&id, |&(a, _)| a) {
                Ok(i) => Handle::model(map[i].1),
                Err(_) => x,
            };
        }
        if !x.is_compound() {
            return x;
        }
        let v = h.view(x);
        let (kind, lo, data) = (v.kind, v.lo, v.data.to_vec());
        let mut img: Vec<Handle> = data.iter().map(|&e| sorted_image(h, map, e)).collect();
        match kind {
            Kind::Set => h.set_from(&mut img),
            Kind::IntFcn => h.int_fcn(lo, &img),
            Kind::Fcn => {
                let n = img.len() / 2;
                let mut pairs: Vec<(Handle, Handle)> =
                    (0..n).map(|i| (img[i], img[n + i])).collect();
                h.fcn_from_pairs(&mut pairs)
            }
            _ => x,
        }
    }

    #[test]
    fn functions_permute_by_domain_images_as_by_sorting() {
        let mut h = Heap::new(Arc::new(Store::default()), false, false);
        let mv = |h: &Heap, t: &str| Handle::model(h.store().strings().intern(t).unwrap());
        // `n0` is not moved by any map, and sorts between the others.
        let vals = [mv(&h, "a"), mv(&h, "m"), mv(&h, "z"), mv(&h, "n0")];
        let ids: Vec<u32> = vals.iter().map(|v| v.as_model_id().unwrap()).collect();
        let perms = [[0, 2, 1], [1, 0, 2], [1, 2, 0], [2, 0, 1], [2, 1, 0]];
        let maps: Vec<Vec<(u32, u32)>> = perms
            .iter()
            .map(|p| (0..3).map(|i| (ids[i], ids[p[i]])).collect())
            .collect();
        let sym = Symmetry::new(maps);
        let int = |n: i64| Handle::int(n).unwrap();
        let s = h.store().strings().intern("f").unwrap();
        let text = Handle::string(s);
        let mut cases = Vec::new();
        for round in 0..3i64 {
            // Keyed by model values: the whole symmetric set, a subset, a
            // domain with an unmoved value, values that are model values.
            let f = h.fcn_from_pairs(&mut [
                (vals[0], int(round)),
                (vals[1], vals[2]),
                (vals[2], int(7)),
            ]);
            let g = h.fcn_from_pairs(&mut [(vals[0], vals[1]), (vals[2], int(round))]);
            let u = h.fcn_from_pairs(&mut [
                (vals[3], vals[0]),
                (vals[1], int(round)),
                (int(5), vals[2]),
            ]);
            // Keyed by pairs of model values, interned (remembered) and not.
            let mut pairs = Vec::new();
            for (i, &x) in vals[..3].iter().enumerate() {
                for (j, &y) in vals[..3].iter().enumerate() {
                    let t = h.tuple(&[x, y]);
                    let t = h.intern(t).unwrap();
                    #[allow(clippy::cast_possible_wrap, reason = "small indexes")]
                    pairs.push((t, int((i * 3 + j) as i64 + round)));
                }
            }
            let p = h.fcn_from_pairs(&mut pairs);
            let p_interned = h.intern(p).unwrap();
            let t0 = h.tuple(&[vals[0], vals[1]]);
            let t1 = h.tuple(&[vals[2], vals[0]]);
            let q = h.fcn_from_pairs(&mut [(t0, int(1)), (t1, vals[1])]);
            // Nested: a function of functions keyed by model values.
            let nested = h.fcn_from_pairs(&mut [(vals[0], g), (vals[1], f), (vals[2], u)]);
            let rec = h.record(&[text], &[vals[0]]);
            cases.extend([f, g, u, p, p_interned, q, nested, rec]);
        }
        for (k, map) in sym.maps.iter().enumerate() {
            let k = u32::try_from(k).unwrap();
            for &x in &cases {
                let want = sorted_image(&mut h, map, x);
                let got = permute_node(&sym, &mut h, map, k, x).unwrap();
                assert!(h.eq(got, want), "map {k}: {x:?}");
            }
        }
        assert!(
            !h.sym_images.domains.is_empty(),
            "domain images are remembered"
        );

        // Compared before it is built: as built, against every other case
        // (functions of other sizes, kinds and domains among them).
        let mut decided = 0;
        for (k, map) in sym.maps.iter().enumerate() {
            let k = u32::try_from(k).unwrap();
            for &x in &cases {
                let img = permute_node(&sym, &mut h, map, k, x).unwrap();
                for &best in &cases {
                    let want = h.cmp(img, best);
                    if let Some(got) = cmp_image(&sym, &mut h, map, k, x, best).unwrap() {
                        assert_eq!(got, want, "map {k}: {x:?} against {best:?}");
                        decided += 1;
                    }
                }
            }
        }
        assert_eq!(decided, sym.len() * cases.len() * cases.len());
        // The canonical image of states of these functions is the least
        // image built by sorting.
        for w in cases.windows(3) {
            let mut want = w.to_vec();
            for map in &sym.maps {
                let img: Vec<Handle> = w.iter().map(|&x| sorted_image(&mut h, map, x)).collect();
                let ord = img
                    .iter()
                    .zip(&want)
                    .map(|(&x, &y)| h.cmp(x, y))
                    .find(|&c| c != Ordering::Equal);
                if ord == Some(Ordering::Less) {
                    want = img;
                }
            }
            let mut got = Vec::new();
            sym.canonical(&mut h, w, &mut got).unwrap();
            assert!(got.iter().zip(&want).all(|(&x, &y)| h.eq(x, y)));
        }
    }

    #[test]
    fn images_keep_record_and_tuple_literals() {
        let mut h = Heap::new(Arc::new(Store::default()), false, false);
        let mv = |h: &Heap, t: &str| Handle::model(h.store().strings().intern(t).unwrap());
        let (a, b) = (mv(&h, "a"), mv(&h, "b"));
        let swap = vec![
            (a.as_model_id().unwrap(), b.as_model_id().unwrap()),
            (b.as_model_id().unwrap(), a.as_model_id().unwrap()),
        ];
        let sym = Symmetry::new(vec![swap.clone()]);
        let field = Handle::string(h.store().strings().intern("x").unwrap());
        let rec = h.record(&[field], &[a]);
        let img = permute_node(&sym, &mut h, &swap, 0, rec).unwrap();
        let want = h.record(&[field], &[b]);
        assert!(h.eq(img, want));
        assert!(h.view(img).is_record_literal());
        let tup = h.tuple_literal(&[a, b]);
        let img = permute_node(&sym, &mut h, &swap, 0, tup).unwrap();
        assert!(h.view(img).is_tuple_literal());
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
