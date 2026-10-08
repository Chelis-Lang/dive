//! Equality, the total order, and normalization.
//!
//! # The order
//!
//! [`Value::cmp_with`] is a total order that extends TLC's partial order
//! (`Value.compareTo` in `tlc2.value.impl`). Wherever TLC compares two values without
//! error, the two agree; where TLC raises an error, the total order falls back to a
//! fixed ranking of kinds:
//!
//! 1. model values (by name),
//! 2. booleans (`FALSE < TRUE`),
//! 3. integers (numerically),
//! 4. strings (by name),
//! 5. finite sets (by cardinality, then element by element in normalized order:
//!    `SetEnumValue.compareTo`),
//! 6. functions (by domain size, then domain element by element, then values element
//!    by element: `FcnRcdValue.compareTo`; tuples and records are functions),
//! 7. infinite or unenumerable sets (`Nat < Int < STRING < Seq(S)`, then by definition).
//!
//! Names (rules 1 and 4, and record fields) are ordered by [`ValueOrder`]: TLC's intern
//! order or dive's canonical order.
//!
//! # Normalization and TLC's errors
//!
//! TLC sorts a set with a binary insertion sort (`ValueVec.sort`) and raises an error
//! when it compares two incomparable values. Whether it raises one depends on which
//! pairs the sort happens to compare. dive reproduces this exactly: when every pair of
//! elements is guaranteed comparable (checked by a cheap structural "shape" join),
//! dive uses a fast stable sort, which gives TLC's result; otherwise it runs TLC's
//! insertion sort with TLC's strict comparison, which gives TLC's result or TLC's error.

#![allow(
    clippy::many_single_char_names,
    reason = "ports keep short operand names (a, b, s, t, x, y)"
)]

use std::cmp::Ordering;

use crate::atom::Atom;
use crate::ctx::{Ctx, IntMode, ValueOrder};
use crate::error::{ErrorKind, Result, ValueError};
use crate::value::{Fcn, FcnRepr, Set, SetRepr, Value};

fn enum_ctx(order: ValueOrder) -> Ctx {
    Ctx {
        int_mode: IntMode::Big,
        order,
    }
}

/// The semantic equality behind `impl PartialEq for Value`. Total: values TLC refuses
/// to compare are simply unequal. Use [`crate::tlc::equals`] for TLC's checked equality.
pub(crate) fn semantic_eq(a: &Value, b: &Value) -> bool {
    match (a, b) {
        (Value::Bool(x), Value::Bool(y)) => x == y,
        (Value::Int(x), Value::Int(y)) => x == y,
        (Value::BigInt(x), Value::BigInt(y)) => x == y,
        (Value::Str(x), Value::Str(y)) | (Value::Model(x), Value::Model(y)) => x == y,
        (Value::Set(x), Value::Set(y)) => x.ptr_eq(y) || (x.hash64() == y.hash64() && set_eq(x, y)),
        (Value::Fcn(x), Value::Fcn(y)) => x.ptr_eq(y) || (x.0.hash == y.0.hash && fcn_eq(x, y)),
        _ => false,
    }
}

fn set_eq(x: &Set, y: &Set) -> bool {
    let ex = x.elements(enum_ctx(x.order()));
    let ey = y.elements(enum_ctx(y.order()));
    match (ex, ey) {
        (Ok(a), Ok(b)) => {
            a.len() == b.len()
                && if x.order() == y.order() {
                    a.iter().zip(b.iter()).all(|(p, q)| semantic_eq(p, q))
                } else {
                    a.iter().all(|p| b.iter().any(|q| semantic_eq(p, q)))
                }
        }
        (Err(_), Err(_)) => structural_eq(x.repr(), y.repr()),
        _ => false,
    }
}

fn structural_eq(a: &SetRepr, b: &SetRepr) -> bool {
    match (a, b) {
        (SetRepr::Nat, SetRepr::Nat)
        | (SetRepr::Int, SetRepr::Int)
        | (SetRepr::Strings, SetRepr::Strings) => true,
        (SetRepr::Seq(x), SetRepr::Seq(y)) | (SetRepr::Subset(x), SetRepr::Subset(y)) => x == y,
        (SetRepr::FcnSet(d1, r1), SetRepr::FcnSet(d2, r2)) => d1 == d2 && r1 == r2,
        (SetRepr::RcdSet(f1), SetRepr::RcdSet(f2)) => {
            f1.len() == f2.len() && f1.iter().zip(f2.iter()).all(|(p, q)| p == q)
        }
        (SetRepr::Product(f1), SetRepr::Product(f2)) => f1 == f2,
        (SetRepr::Interval(l1, h1), SetRepr::Interval(l2, h2)) => l1 == l2 && h1 == h2,
        (SetRepr::Deferred(p), SetRepr::Deferred(q)) => std::sync::Arc::ptr_eq(p, q),
        _ => false,
    }
}

fn fcn_eq(x: &Fcn, y: &Fcn) -> bool {
    match (&x.0.repr, &y.0.repr) {
        (FcnRepr::Tuple(a), FcnRepr::Tuple(b)) => a == b,
        (FcnRepr::Record(a), FcnRepr::Record(b)) => a == b,
        (FcnRepr::General(a), FcnRepr::General(b)) => a == b,
        _ => false,
    }
}

fn rank(v: &Value) -> u8 {
    match v {
        Value::Model(_) => 0,
        Value::Bool(_) => 1,
        Value::Int(_) | Value::BigInt(_) => 2,
        Value::Str(_) => 3,
        Value::Set(s) => {
            if s.elements(enum_ctx(s.order())).is_ok() {
                4
            } else {
                6
            }
        }
        Value::Fcn(_) => 5,
    }
}

impl Value {
    /// A total order on all values, under the name order `order`.
    ///
    /// It extends TLC's partial order (`Value.compareTo` in `tlc2.value.impl`):
    /// wherever TLC compares two values without error the two agree, and where TLC
    /// raises an error this order falls back to a fixed ranking of kinds:
    ///
    /// 1. model values (by name),
    /// 2. booleans (`FALSE < TRUE`),
    /// 3. integers (numerically),
    /// 4. strings (by name),
    /// 5. finite sets (by cardinality, then element by element in normalized order),
    /// 6. functions, including tuples and records (by domain size, then domain
    ///    element by element, then values element by element),
    /// 7. infinite or unenumerable sets (`Nat < Int < STRING < Seq(S)`, then by
    ///    definition).
    ///
    /// Names (rules 1 and 4, and record fields) are ordered by [`ValueOrder`].
    /// Consistent with `==`: `a.cmp_with(b, o) == Equal` iff `a == b`.
    #[must_use]
    pub fn cmp_with(&self, other: &Self, order: ValueOrder) -> Ordering {
        total_cmp(self, other, order)
    }
}

pub(crate) fn total_cmp(a: &Value, b: &Value, order: ValueOrder) -> Ordering {
    match (a, b) {
        (Value::Bool(x), Value::Bool(y)) => return x.cmp(y),
        (Value::Int(x), Value::Int(y)) => return x.cmp(y),
        (Value::Str(x), Value::Str(y)) | (Value::Model(x), Value::Model(y)) => {
            return x.cmp_with(y, order);
        }
        _ => {}
    }
    let (ra, rb) = (rank(a), rank(b));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (a, b) {
        (Value::Int(_) | Value::BigInt(_), Value::Int(_) | Value::BigInt(_)) => {
            match (a.as_int(), b.as_int()) {
                (Some(x), Some(y)) => x.cmp(&y),
                _ => Ordering::Equal,
            }
        }
        (Value::Set(x), Value::Set(y)) => {
            if x.ptr_eq(y) {
                return Ordering::Equal;
            }
            if ra == 4 {
                match (x.elements(enum_ctx(order)), y.elements(enum_ctx(order))) {
                    (Ok(ex), Ok(ey)) => seq_cmp(&ex, &ey, order),
                    _ => Ordering::Equal,
                }
            } else {
                infinite_cmp(x.repr(), y.repr(), order)
            }
        }
        (Value::Fcn(x), Value::Fcn(y)) => {
            if x.ptr_eq(y) {
                return Ordering::Equal;
            }
            x.len()
                .cmp(&y.len())
                .then_with(|| {
                    for i in 0..x.len() {
                        if let (Some((dx, _)), Some((dy, _))) = (x.entry(i), y.entry(i)) {
                            let c = total_cmp(&dx, &dy, order);
                            if c != Ordering::Equal {
                                return c;
                            }
                        }
                    }
                    Ordering::Equal
                })
                .then_with(|| {
                    for i in 0..x.len() {
                        if let (Some((_, vx)), Some((_, vy))) = (x.entry(i), y.entry(i)) {
                            let c = total_cmp(vx, vy, order);
                            if c != Ordering::Equal {
                                return c;
                            }
                        }
                    }
                    Ordering::Equal
                })
        }
        _ => Ordering::Equal,
    }
}

fn seq_cmp(a: &[Value], b: &[Value], order: ValueOrder) -> Ordering {
    a.len().cmp(&b.len()).then_with(|| {
        for (x, y) in a.iter().zip(b) {
            let c = total_cmp(x, y, order);
            if c != Ordering::Equal {
                return c;
            }
        }
        Ordering::Equal
    })
}

fn infinite_rank(r: &SetRepr) -> u8 {
    match r {
        SetRepr::Nat => 0,
        SetRepr::Int => 1,
        SetRepr::Strings => 2,
        SetRepr::Seq(_) => 3,
        SetRepr::Subset(_) => 4,
        SetRepr::FcnSet(..) => 5,
        SetRepr::RcdSet(_) => 6,
        SetRepr::Product(_) => 7,
        SetRepr::Interval(..) => 8,
        SetRepr::Enum | SetRepr::Raw(_) => 9,
        SetRepr::Deferred(_) => 10,
    }
}

fn infinite_cmp(a: &SetRepr, b: &SetRepr, order: ValueOrder) -> Ordering {
    let (ra, rb) = (infinite_rank(a), infinite_rank(b));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (a, b) {
        (SetRepr::Seq(x), SetRepr::Seq(y)) | (SetRepr::Subset(x), SetRepr::Subset(y)) => {
            total_cmp(x, y, order)
        }
        (SetRepr::FcnSet(d1, r1), SetRepr::FcnSet(d2, r2)) => {
            total_cmp(d1, d2, order).then_with(|| total_cmp(r1, r2, order))
        }
        (SetRepr::RcdSet(f1), SetRepr::RcdSet(f2)) => f1.len().cmp(&f2.len()).then_with(|| {
            for ((n1, s1), (n2, s2)) in f1.iter().zip(f2.iter()) {
                let c = n1
                    .cmp_with(n2, order)
                    .then_with(|| total_cmp(s1, s2, order));
                if c != Ordering::Equal {
                    return c;
                }
            }
            Ordering::Equal
        }),
        (SetRepr::Product(f1), SetRepr::Product(f2)) => seq_cmp(f1, f2, order),
        (SetRepr::Interval(l1, h1), SetRepr::Interval(l2, h2)) => {
            l1.cmp(l2).then_with(|| h1.cmp(h2))
        }
        (SetRepr::Deferred(p), SetRepr::Deferred(q)) => {
            // Unenumerable evaluator sets have no observable order; order by identity.
            let pa = std::sync::Arc::as_ptr(p).cast::<()>() as usize;
            let qa = std::sync::Arc::as_ptr(q).cast::<()>() as usize;
            pa.cmp(&qa)
        }
        _ => Ordering::Equal,
    }
}

// ---------------------------------------------------------------------------------
// Comparability shapes
// ---------------------------------------------------------------------------------

/// A conservative description of which TLC comparisons can fail. If the join of two
/// values' shapes is not `Conflict`, TLC compares them without error.
#[derive(Clone, PartialEq, Eq, Debug)]
enum Shape {
    Bottom,
    AnyMv,
    TypedMv(u16),
    Bool,
    Int,
    Str,
    Set(Box<Shape>),
    Fcn(Box<Shape>, Box<Shape>),
    Conflict,
}

fn join(a: Shape, b: Shape) -> Shape {
    use Shape::{AnyMv, Bottom, Conflict, Fcn, Set, TypedMv};
    match (a, b) {
        (Conflict, _) | (_, Conflict) => Conflict,
        (Bottom | AnyMv, x) | (x, Bottom | AnyMv) => x,
        (TypedMv(s), TypedMv(t)) => {
            if s == t {
                TypedMv(s)
            } else {
                Conflict
            }
        }
        (Set(x), Set(y)) => match join(*x, *y) {
            Conflict => Conflict,
            j => Set(Box::new(j)),
        },
        (Fcn(d1, v1), Fcn(d2, v2)) => match (join(*d1, *d2), join(*v1, *v2)) {
            (Conflict, _) | (_, Conflict) => Conflict,
            (d, v) => Fcn(Box::new(d), Box::new(v)),
        },
        (x, y) => {
            if x == y && matches!(x, Shape::Bool | Shape::Int | Shape::Str) {
                x
            } else {
                Conflict
            }
        }
    }
}

fn join_all<'a>(vals: impl Iterator<Item = &'a Value>) -> Shape {
    let mut acc = Shape::Bottom;
    for v in vals {
        acc = join(acc, shape(v));
        if acc == Shape::Conflict {
            break;
        }
    }
    acc
}

fn shape(v: &Value) -> Shape {
    match v {
        Value::Bool(_) => Shape::Bool,
        Value::Int(_) | Value::BigInt(_) => Shape::Int,
        Value::Str(_) => Shape::Str,
        Value::Model(a) => a.mv_type().map_or(Shape::AnyMv, Shape::TypedMv),
        Value::Set(s) => match s.repr() {
            SetRepr::Enum => match s.0.elems.get() {
                Some(Ok(e)) => match join_all(e.iter()) {
                    Shape::Conflict => Shape::Conflict,
                    j => Shape::Set(Box::new(j)),
                },
                _ => Shape::Conflict,
            },
            SetRepr::Interval(..) => Shape::Set(Box::new(Shape::Int)),
            _ => Shape::Conflict,
        },
        Value::Fcn(f) => match &f.0.repr {
            FcnRepr::Tuple(e) => {
                let d = if e.is_empty() {
                    Shape::Bottom
                } else {
                    Shape::Int
                };
                match join_all(e.iter()) {
                    Shape::Conflict => Shape::Conflict,
                    j => Shape::Fcn(Box::new(d), Box::new(j)),
                }
            }
            FcnRepr::Record(fs) => match join_all(fs.iter().map(|(_, v)| v)) {
                Shape::Conflict => Shape::Conflict,
                j => Shape::Fcn(Box::new(Shape::Str), Box::new(j)),
            },
            FcnRepr::General(ps) => {
                match (
                    join_all(ps.iter().map(|(d, _)| d)),
                    join_all(ps.iter().map(|(_, v)| v)),
                ) {
                    (Shape::Conflict, _) | (_, Shape::Conflict) => Shape::Conflict,
                    (d, v) => Shape::Fcn(Box::new(d), Box::new(v)),
                }
            }
        },
    }
}

/// Whether every pair of `elems` is comparable in TLC without error.
fn all_comparable(elems: &[Value]) -> bool {
    // Fast scalar pass first: the common sets of integers or strings need no shapes.
    let Some(first) = elems.first() else {
        return true;
    };
    let scalar_same = match first {
        Value::Int(_) => elems
            .iter()
            .all(|v| matches!(v, Value::Int(_) | Value::BigInt(_))),
        Value::Str(_) => elems.iter().all(|v| matches!(v, Value::Str(_))),
        Value::Bool(_) => elems.iter().all(|v| matches!(v, Value::Bool(_))),
        _ => false,
    };
    scalar_same || join_all(elems.iter()) != Shape::Conflict
}

// ---------------------------------------------------------------------------------
// Normalization
// ---------------------------------------------------------------------------------

/// Sorts and deduplicates set elements as TLC's `SetEnumValue.normalize` does.
pub(crate) fn normalize_set(mut elems: Vec<Value>, ctx: Ctx) -> Result<Vec<Value>> {
    if elems.len() <= 1 {
        return Ok(elems);
    }
    if all_comparable(&elems) {
        // Enumerations inside the comparison (lazy elements) cannot fail here: a
        // `Conflict`-free shape contains only enumerated sets and intervals.
        elems.sort_by(|a, b| total_cmp(a, b, ctx.order));
        elems.dedup_by(|later, earlier| total_cmp(later, earlier, ctx.order) == Ordering::Equal);
        return Ok(elems);
    }
    tlc_insertion_sort(elems, ctx)
}

/// `ValueVec.sort(true)`: binary insertion with `elem.compareTo(sorted[mid])`, keeping
/// the first of equal elements.
fn tlc_insertion_sort(elems: Vec<Value>, ctx: Ctx) -> Result<Vec<Value>> {
    let mut iter = elems.into_iter();
    let mut out: Vec<Value> = Vec::new();
    if let Some(first) = iter.next() {
        out.push(first);
    }
    for elem in iter {
        let (mut low, mut high) = (0usize, out.len());
        let mut cmp = Ordering::Equal;
        let mut idx = 0usize;
        while low < high {
            idx = low + ((high - low) >> 1);
            cmp = crate::tlc::compare(&elem, &out[idx], ctx)?;
            match cmp {
                Ordering::Equal => break,
                Ordering::Less => high = idx,
                Ordering::Greater => low = idx + 1,
            }
        }
        if cmp != Ordering::Equal {
            let at = if cmp == Ordering::Less { idx } else { idx + 1 };
            out.insert(at, elem);
        }
    }
    Ok(out)
}

fn dup_domain(v: &Value, ctx: Ctx) -> ValueError {
    ValueError::new(
        ErrorKind::Duplicate,
        format!(
            "The value\n{}\noccurs multiple times in the function domain.",
            crate::print::to_tlc_string_lenient(v, ctx)
        ),
    )
}

/// Builds a function in canonical form (see [`Fcn`]), normalizing the domain as
/// `FcnRcdValue.normalize` does.
pub(crate) fn make_fcn(mut pairs: Vec<(Value, Value)>, ctx: Ctx) -> Result<Value> {
    if pairs.is_empty() {
        return Ok(Value::tuple(Vec::new()));
    }
    if pairs.len() > 1 {
        let doms: Vec<Value> = pairs.iter().map(|(d, _)| d.clone()).collect();
        if all_comparable(&doms) {
            pairs.sort_by(|a, b| total_cmp(&a.0, &b.0, ctx.order));
            for w in pairs.windows(2) {
                if total_cmp(&w[0].0, &w[1].0, ctx.order) == Ordering::Equal {
                    return Err(dup_domain(&w[1].0, ctx));
                }
            }
        } else {
            tlc_fcn_normalize(&mut pairs, ctx)?;
        }
    }
    // Canonical representation by domain.
    let n = pairs.len();
    let is_tuple = pairs.iter().enumerate().all(
        |(i, (d, _))| matches!(d, Value::Int(k) if i64::try_from(i).is_ok_and(|i| *k == i + 1)),
    );
    if is_tuple {
        return Ok(Value::tuple(pairs.into_iter().map(|(_, v)| v).collect()));
    }
    if pairs.iter().all(|(d, _)| matches!(d, Value::Str(_))) {
        let mut fields: Vec<(Atom, Value)> = Vec::with_capacity(n);
        for (d, v) in pairs {
            if let Value::Str(a) = d {
                fields.push((a, v));
            }
        }
        return Ok(Value::Fcn(Fcn::record(fields)));
    }
    Ok(Value::Fcn(Fcn::general(pairs)))
}

/// `FcnRcdValue.normalize`: bring the minimum to the front by swaps, then insertion
/// sort from index 2, comparing with TLC's strict `compareTo`.
fn tlc_fcn_normalize(pairs: &mut [(Value, Value)], ctx: Ctx) -> Result<()> {
    let n = pairs.len();
    for i in 1..n {
        let c = crate::tlc::compare(&pairs[0].0, &pairs[i].0, ctx)?;
        if c == Ordering::Equal {
            return Err(dup_domain(&pairs[i].0, ctx));
        }
        if c == Ordering::Greater {
            pairs.swap(0, i);
        }
    }
    for i in 2..n {
        let item = pairs[i].clone();
        let mut j = i;
        let mut c = Ordering::Less;
        while j > 0 {
            c = crate::tlc::compare(&item.0, &pairs[j - 1].0, ctx)?;
            if c != Ordering::Less {
                break;
            }
            pairs[j] = pairs[j - 1].clone();
            j -= 1;
        }
        if c == Ordering::Equal {
            // TLC prints `this.domain[i]` after the shifts, which may be a neighbour.
            return Err(dup_domain(&pairs[i].0, ctx));
        }
        pairs[j] = item;
    }
    Ok(())
}
