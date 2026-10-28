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

use crate::Error;
use crate::handle::{Handle, Tag};
use crate::heap::Heap;
use crate::node::Kind;

/// The closure's nonidentity maps, each as its moved model-value ids
/// `(from, to)`, sorted by `from`.
#[derive(Clone, Debug, Default)]
pub struct Symmetry {
    maps: Vec<Vec<(u32, u32)>>,
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
        Self { maps, tlc: None }
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
    /// strictly smaller. The images are transient values on `heap`.
    ///
    /// # Errors
    /// A value the core cannot permute (unsupported).
    pub fn canonical(
        &self,
        heap: &mut Heap,
        state: &[Handle],
        out: &mut Vec<Handle>,
    ) -> Result<(), Error> {
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
        for map in &self.maps {
            let mut ord = Ordering::Equal;
            for &i in order {
                let (Some(&h), Some(&best)) = (state.get(i), out.get(i)) else {
                    return Err(Error::internal("a SYMMETRY variable outside the state"));
                };
                let img = permute(heap, map, h)?;
                cand[i] = img;
                if ord == Ordering::Equal {
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
            if ord == Ordering::Less {
                out.clone_from(&cand);
            }
        }
        Ok(())
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

/// The image of `h` under `map`; `h` itself when nothing in it moves.
#[allow(
    clippy::many_single_char_names,
    reason = "short names for a value, its view and its entries"
)]
fn permute(heap: &mut Heap, map: &[(u32, u32)], h: Handle) -> Result<Handle, Error> {
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
                let x = permute(heap, map, e)?;
                moved |= x != e;
                elems.push(x);
            }
            Ok(if moved { heap.set_from(&mut elems) } else { h })
        }
        Kind::IntFcn => {
            let mut moved = false;
            let mut vals = Vec::with_capacity(data.len());
            for e in data {
                let x = permute(heap, map, e)?;
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
                let (k, e) = (data[i], data[n + i]);
                let (pk, pe) = (permute(heap, map, k)?, permute(heap, map, e)?);
                moved |= pk != k || pe != e;
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
