//! Streaming a set's elements in normalized order, without materializing lazy sets.
//!
//! Each lazy set is generated directly in the order normalization would produce (as
//! TLC's `toSetEnum` does): intervals ascending, `SUBSET S` by size and then
//! lexicographically, and `[S -> T]`, `S \X T` and `[a : S]` as odometers whose last
//! position varies fastest. Only the components (`S`, `T`) are materialized.

use std::sync::Arc;

use crate::atom::Atom;
use crate::ctx::{Ctx, IntMode, ValueOrder};
use crate::error::{ErrorKind, Result, ValueError};
use crate::int::Int;
use crate::value::{Fcn, Set, SetRepr, Value};

/// A stream of elements in normalized order.
pub(crate) type Elems = Box<dyn Iterator<Item = Value> + Send>;

pub(crate) fn not_enumerable(what: &Value, ctx: Ctx) -> ValueError {
    ValueError::new(
        ErrorKind::NotEnumerable,
        format!(
            "Attempted to enumerate the infinite or non-enumerable set\n{}",
            crate::print::ppr(&crate::print::to_tlc_string_lenient(what, ctx))
        ),
    )
}

/// The elements of a set-valued component, sorted in `order`. `mark` records the
/// normalization of an unnormalized component, as TLC's enumeration does.
pub(crate) fn component(v: &Value, ctx: Ctx, mark: bool) -> Result<Arc<[Value]>> {
    match v {
        Value::Set(s) => elems_in(s, ctx, mark),
        _ => Err(ValueError::new(
            ErrorKind::NotEnumerable,
            format!(
                "Attempted to enumerate the non-set value\n{}",
                crate::print::ppr(&crate::print::to_tlc_string_lenient(v, ctx))
            ),
        )),
    }
}

/// A set's elements sorted in `ctx.order` (re-sorted if the set was built in another
/// order).
pub(crate) fn elems_in(s: &Set, ctx: Ctx, mark: bool) -> Result<Arc<[Value]>> {
    let e = if mark {
        s.elements(ctx)?
    } else {
        s.quiet_elements(ctx)?
    };
    if s.order() == ctx.order {
        return Ok(e);
    }
    let mut v = e.to_vec();
    v.sort_by(|a, b| a.cmp_with(b, ctx.order));
    Ok(v.into())
}

struct SliceIter {
    elems: Arc<[Value]>,
    next: usize,
}

impl Iterator for SliceIter {
    type Item = Value;
    fn next(&mut self) -> Option<Value> {
        let v = self.elems.get(self.next).cloned();
        self.next += 1;
        v
    }
}

pub(crate) fn slice_stream(elems: Arc<[Value]>) -> Elems {
    Box::new(SliceIter { elems, next: 0 })
}

struct IntervalIter {
    next: Int,
    hi: Int,
}

impl Iterator for IntervalIter {
    type Item = Value;
    fn next(&mut self) -> Option<Value> {
        if self.next > self.hi {
            return None;
        }
        let v = Value::int(self.next.clone());
        self.next = match &self.next {
            Int::Small(i) if *i < i64::MAX => Int::Small(i + 1),
            other => other.add(&Int::ONE, IntMode::Big).ok()?,
        };
        Some(v)
    }
}

struct SubsetIter {
    base: Arc<[Value]>,
    order: ValueOrder,
    k: usize,
    idx: Vec<usize>,
    started: bool,
}

impl Iterator for SubsetIter {
    type Item = Value;
    fn next(&mut self) -> Option<Value> {
        let m = self.base.len();
        if self.started {
            // Advance the rightmost index that can still move right, or go to k + 1.
            let k = self.k;
            if let Some(pos) = (0..k).rev().find(|&i| self.idx[i] < m - k + i) {
                self.idx[pos] += 1;
                for j in pos + 1..k {
                    self.idx[j] = self.idx[j - 1] + 1;
                }
            } else {
                if self.k >= m {
                    return None;
                }
                self.k += 1;
                self.idx = (0..self.k).collect();
            }
        }
        self.started = true;
        let elems = self.idx.iter().map(|&i| self.base[i].clone()).collect();
        Some(Value::Set(Set::from_sorted(self.order, elems)))
    }
}

/// How an odometer turns a combination of component elements into an element.
#[derive(Clone)]
enum Build {
    Tuple,
    /// A record literal with these fields (from `[a : S]`).
    Record(Arc<[Atom]>, ValueOrder),
    /// A function with this domain (from `[S -> T]`), canonical by domain shape.
    Fcn(Arc<[Value]>, FcnShape, ValueOrder),
}

#[derive(Clone, Copy)]
enum FcnShape {
    Tuple,
    Record,
    General,
}

struct Odometer {
    comps: Vec<Arc<[Value]>>,
    idx: Vec<usize>,
    done: bool,
    build: Build,
}

impl Iterator for Odometer {
    type Item = Value;
    fn next(&mut self) -> Option<Value> {
        if self.done {
            return None;
        }
        let vals: Vec<Value> = self
            .comps
            .iter()
            .zip(&self.idx)
            .map(|(c, &i)| c[i].clone())
            .collect();
        // Advance, last position fastest.
        let mut pos = self.comps.len();
        loop {
            if pos == 0 {
                self.done = true;
                break;
            }
            pos -= 1;
            self.idx[pos] += 1;
            if self.idx[pos] < self.comps[pos].len() {
                break;
            }
            self.idx[pos] = 0;
        }
        Some(match &self.build {
            Build::Tuple => Value::tuple(vals),
            Build::Record(names, order) => Value::Fcn(Fcn::record(
                names.iter().cloned().zip(vals).collect(),
                *order,
                true,
            )),
            Build::Fcn(dom, shape, order) => match shape {
                FcnShape::Tuple => Value::tuple_fcn(vals),
                FcnShape::Record => Value::Fcn(Fcn::record(
                    dom.iter()
                        .filter_map(|d| match d {
                            Value::Str(a) => Some(a.clone()),
                            _ => None,
                        })
                        .zip(vals)
                        .collect(),
                    *order,
                    false,
                )),
                FcnShape::General => Value::Fcn(Fcn::general(
                    dom.iter().cloned().zip(vals).collect(),
                    *order,
                )),
            },
        })
    }
}

fn odometer(comps: Vec<Arc<[Value]>>, build: Build) -> Elems {
    let done = comps.iter().any(|c| c.is_empty());
    let idx = vec![0; comps.len()];
    Box::new(Odometer {
        comps,
        idx,
        done,
        build,
    })
}

fn fcn_shape(dom: &[Value]) -> FcnShape {
    let tuple = dom
        .iter()
        .enumerate()
        .all(|(i, d)| matches!(d, Value::Int(k) if i64::try_from(i).is_ok_and(|i| *k == i + 1)));
    if tuple {
        FcnShape::Tuple
    } else if dom.iter().all(|d| matches!(d, Value::Str(_))) {
        FcnShape::Record
    } else {
        FcnShape::General
    }
}

/// Streams the elements of `s` in normalized order (under `s`'s own order).
///
/// `mark` selects TLC's view: with `mark`, unnormalized sets met on the way count as
/// normalized afterwards (TLC's enumeration normalizes them); without it, nothing
/// observable changes.
pub(crate) fn stream(s: &Set, ctx: Ctx, mark: bool) -> Result<Elems> {
    let ctx = Ctx {
        order: s.order(),
        ..ctx
    };
    let me = || Value::Set(s.clone());
    Ok(match s.repr() {
        SetRepr::Enum | SetRepr::Raw(_) | SetRepr::Deferred(_) => {
            let e = if mark {
                s.elements(ctx)?
            } else {
                s.quiet_elements(ctx)?
            };
            slice_stream(e)
        }
        SetRepr::Interval(lo, hi) => Box::new(IntervalIter {
            next: lo.clone(),
            hi: hi.clone(),
        }),
        SetRepr::Subset(b) => Box::new(SubsetIter {
            base: component(b, ctx, mark)?,
            order: ctx.order,
            k: 0,
            idx: Vec::new(),
            started: false,
        }),
        SetRepr::FcnSet(d, r) => {
            // SetOfFcnsValue.elements tests emptiness before enumerating the
            // domain. Keep quiet operations from normalizing nested operands.
            let _quiet = (!mark).then(crate::value::quiet::Quiet::enter);
            if crate::tlc::is_empty(&me(), ctx)? {
                return Ok(Box::new(std::iter::empty()));
            }
            let dom = component(d, ctx, mark)?;
            if dom.is_empty() {
                return Ok(Box::new(std::iter::once(Value::tuple(Vec::new()))));
            }
            let rng = component(r, ctx, mark)?;
            let shape = fcn_shape(&dom);
            odometer(vec![rng; dom.len()], Build::Fcn(dom, shape, ctx.order))
        }
        SetRepr::RcdSet(fs) => {
            let mut comps = Vec::with_capacity(fs.len());
            for (_, v) in fs {
                comps.push(component(v, ctx, mark)?);
            }
            let names: Arc<[Atom]> = fs.iter().map(|(a, _)| a.clone()).collect();
            odometer(comps, Build::Record(names, ctx.order))
        }
        SetRepr::Product(fs) => {
            let mut comps = Vec::with_capacity(fs.len());
            for v in fs {
                comps.push(component(v, ctx, mark)?);
            }
            odometer(comps, Build::Tuple)
        }
        SetRepr::Seq(r) => {
            if component(r, ctx, mark).is_ok_and(|e| e.is_empty()) {
                Box::new(std::iter::once(Value::tuple(Vec::new())))
            } else {
                return Err(not_enumerable(&me(), ctx));
            }
        }
        SetRepr::Nat | SetRepr::Int | SetRepr::Strings => return Err(not_enumerable(&me(), ctx)),
    })
}
