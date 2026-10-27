//! Symmetry reduction (`SYMMETRY`): the state key of a state is the key of
//! its canonical image, the least of its images under the closure's
//! permutations of model values, compared variable by variable in the
//! run's value order. The explored state stays the concrete one; only keys
//! change, as in the reference engine
//! (`crates/dive-model/src/symmetry/prepare.rs`, `Reduction::canonical_values`).
//!
//! The core implements the default profile's canonicalization (D-010: the
//! canonical value order, variables in declaration order). Values it cannot
//! permute exactly, lazily represented sets and functions, are refused.

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
}

impl Symmetry {
    /// The reduction from the maps' moved pairs of model-value ids.
    #[must_use]
    pub fn new(mut maps: Vec<Vec<(u32, u32)>>) -> Self {
        for m in &mut maps {
            m.retain(|(a, b)| a != b);
            m.sort_unstable();
        }
        Self { maps }
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
        let mut cand = Vec::with_capacity(state.len());
        for map in &self.maps {
            cand.clear();
            let mut ord = Ordering::Equal;
            for (i, &h) in state.iter().enumerate() {
                let img = permute(heap, map, h)?;
                cand.push(img);
                if ord == Ordering::Equal {
                    ord = heap.cmp(img, out[i]);
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
}
