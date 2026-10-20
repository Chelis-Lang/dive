//! Equality, the total order, hashing of sets, and normalization.
//!
//! # Equality and the order
//!
//! `impl PartialEq for Value` and [`Value::cmp_with`] are total and consistent:
//! `a.cmp_with(b, o) == Equal` iff `a == b`, and equal values hash equally. Both are
//! iterative, so any nesting depth is safe, and neither changes anything TLC can
//! observe (unnormalized sets stay unnormalized).
//!
//! Sets fall in three classes:
//! - **intervals**, compared by their bounds where possible;
//! - **finite** sets of at most [`MAX_ENUMERATION`] elements (enumerated sets, lazy
//!   sets of known size, normalizable set literals), compared by cardinality and then
//!   element by element in normalized order, streaming lazy sets rather than
//!   materializing them;
//! - **structural** sets (infinite, too large, or unenumerable, such as a literal
//!   `{1, "a"}` that TLC cannot normalize), compared by their definition.
//!
//! # Normalization and TLC's errors
//!
//! TLC sorts a set with a binary insertion sort (`ValueVec.sort`) and raises an error
//! when it compares two incomparable values. Whether it raises one depends on which
//! pairs the sort happens to compare. dive reproduces this exactly: when every pair of
//! elements is provably comparable (a cheap structural "shape" join), dive uses a fast
//! stable sort, which gives TLC's result; otherwise it runs TLC's insertion sort with
//! TLC's strict comparison, in TLC's name order, which gives TLC's result or TLC's
//! error, and then sorts the result in the run's order.

#![allow(
    clippy::many_single_char_names,
    reason = "ports keep short operand names (a, b, s, t, x, y)"
)]

use std::borrow::Cow;
use std::cmp::Ordering;

use crate::atom::Atom;
use crate::ctx::{Ctx, IntMode, ValueOrder};
use crate::enumerate::{MAX_ENUMERATION, quiet_size};
use crate::error::{ErrorKind, Result, ValueError};
use crate::stream::{Elems, slice_stream, stream};
use crate::value::{Fcn, FcnRepr, HASH_ELEMENT_LIMIT, K_SET, Set, SetRepr, Value, mix2, set_hash};

fn quiet_ctx(order: ValueOrder) -> Ctx {
    Ctx {
        int_mode: IntMode::Big,
        order,
    }
}

// ---------------------------------------------------------------------------------
// Set classes
// ---------------------------------------------------------------------------------

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Interval,
    Finite,
    Structural,
}

fn class(s: &Set) -> Class {
    match s.repr() {
        SetRepr::Interval(..) => Class::Interval,
        SetRepr::Enum => Class::Finite,
        SetRepr::Nat | SetRepr::Int | SetRepr::Strings => Class::Structural,
        _ => match quiet_size(s) {
            Some(n) if n <= u128::from(MAX_ENUMERATION) => Class::Finite,
            _ => Class::Structural,
        },
    }
}

/// The elements of a finite set in `order`, streamed (or materialized and re-sorted
/// if the set was built in another order). Changes nothing observable.
fn quiet_stream(s: &Set, order: ValueOrder) -> Option<Elems> {
    if s.order() == order {
        return stream(s, quiet_ctx(order), false).ok();
    }
    let mut v: Vec<Value> = stream(s, quiet_ctx(s.order()), false).ok()?.collect();
    v.sort_by(|a, b| total_cmp(a, b, order));
    Some(slice_stream(v.into()))
}

fn interval_empty(s: &Set) -> bool {
    s.as_interval().is_some_and(|(lo, hi)| hi < lo)
}

/// The set's hash (see [`Value::hash64`]).
pub(crate) fn compute_set_hash(s: &Set) -> u64 {
    match class(s) {
        Class::Interval | Class::Finite => match quiet_size(s) {
            Some(n) if n > HASH_ELEMENT_LIMIT => set_hash(std::iter::empty(), n),
            Some(n) => match stream(s, quiet_ctx(s.order()), false) {
                Ok(e) => set_hash(e.map(|v| v.hash64()), n),
                Err(_) => structural_hash(s),
            },
            None => structural_hash(s),
        },
        Class::Structural => structural_hash(s),
    }
}

fn structural_hash(s: &Set) -> u64 {
    let seq = |init: u64, vals: &mut dyn Iterator<Item = &Value>| {
        vals.fold(init, |h, v| mix2(h, v.hash64()))
    };
    let h = match s.repr() {
        SetRepr::Nat => 1,
        SetRepr::Int => 2,
        SetRepr::Strings => 3,
        SetRepr::Seq(r) => mix2(4, r.hash64()),
        SetRepr::Subset(b) => mix2(5, b.hash64()),
        SetRepr::FcnSet(d, r) => mix2(mix2(6, d.hash64()), r.hash64()),
        SetRepr::RcdSet(fs) => fs
            .iter()
            .fold(7, |h, (a, v)| mix2(mix2(h, a.text_hash()), v.hash64())),
        SetRepr::Product(fs) => seq(8, &mut fs.iter()),
        SetRepr::Raw(e) => seq(9, &mut e.iter()),
        SetRepr::Deferred(d) => seq(mix2(10, d.kind() as u64), &mut d.key().iter()),
        SetRepr::Interval(lo, hi) => mix2(
            mix2(11, Value::int(lo.clone()).hash64()),
            Value::int(hi.clone()).hash64(),
        ),
        SetRepr::Enum => 12,
    };
    mix2(K_SET ^ 0xdead_beef, h)
}

/// Equality of the definitions of two structural sets.
fn structural_eq(a: &Set, b: &Set) -> bool {
    match (a.repr(), b.repr()) {
        (SetRepr::Nat, SetRepr::Nat)
        | (SetRepr::Int, SetRepr::Int)
        | (SetRepr::Strings, SetRepr::Strings) => true,
        (SetRepr::Seq(x), SetRepr::Seq(y)) | (SetRepr::Subset(x), SetRepr::Subset(y)) => x == y,
        (SetRepr::FcnSet(d1, r1), SetRepr::FcnSet(d2, r2)) => d1 == d2 && r1 == r2,
        (SetRepr::RcdSet(f1), SetRepr::RcdSet(f2)) => {
            f1.len() == f2.len() && f1.iter().zip(f2.iter()).all(|(p, q)| p == q)
        }
        (SetRepr::Product(f1), SetRepr::Product(f2)) | (SetRepr::Raw(f1), SetRepr::Raw(f2)) => {
            f1 == f2
        }
        (SetRepr::Interval(l1, h1), SetRepr::Interval(l2, h2)) => l1 == l2 && h1 == h2,
        (SetRepr::Deferred(p), SetRepr::Deferred(q)) => p.kind() == q.kind() && p.key() == q.key(),
        _ => false,
    }
}

fn structural_rank(r: &SetRepr) -> u8 {
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
        SetRepr::Enum => 9,
        SetRepr::Raw(_) => 10,
        SetRepr::Deferred(_) => 11,
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

/// The order of two structural sets, by definition.
fn structural_cmp(a: &Set, b: &Set, order: ValueOrder) -> Ordering {
    let (ra, rb) = (structural_rank(a.repr()), structural_rank(b.repr()));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (a.repr(), b.repr()) {
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
        (SetRepr::Product(f1), SetRepr::Product(f2)) | (SetRepr::Raw(f1), SetRepr::Raw(f2)) => {
            seq_cmp(f1, f2, order)
        }
        (SetRepr::Interval(l1, h1), SetRepr::Interval(l2, h2)) => {
            l1.cmp(l2).then_with(|| h1.cmp(h2))
        }
        (SetRepr::Deferred(p), SetRepr::Deferred(q)) => (p.kind() as u8)
            .cmp(&(q.kind() as u8))
            .then_with(|| seq_cmp(&p.key(), &q.key(), order)),
        _ => Ordering::Equal,
    }
}

// ---------------------------------------------------------------------------------
// ---------------------------------------------------------------------------------
// Equality
// ---------------------------------------------------------------------------------

/// The normalized elements of a set that is (or can cheaply be) materialized, borrowed
/// from the set's cache. Changes nothing observable.
fn materialized_ref(s: &Set) -> Option<&[Value]> {
    match s.repr() {
        SetRepr::Enum | SetRepr::Raw(_) | SetRepr::Deferred(_) => {
            s.quiet_elements(quiet_ctx(s.order())).ok()?;
            s.0.elems.get().map(|e| &**e)
        }
        _ => None,
    }
}

/// Equality of scalars, or `None` when both are compound.
fn scalar_eq(a: &Value, b: &Value) -> Option<bool> {
    Some(match (a, b) {
        (Value::Bool(p), Value::Bool(q)) => p == q,
        (Value::Int(p), Value::Int(q)) => p == q,
        (Value::Int(_) | Value::BigInt(_), Value::Int(_) | Value::BigInt(_)) => {
            a.as_int() == b.as_int()
        }
        (Value::Str(p), Value::Str(q)) | (Value::Model(p), Value::Model(q)) => p == q,
        (Value::Fcn(f), Value::Fcn(g)) => {
            if f.ptr_eq(g) {
                return Some(true);
            }
            if f.0.hash != g.0.hash {
                return Some(false);
            }
            return None;
        }
        (Value::Set(_), Value::Set(_)) => return None,
        _ => false,
    })
}

/// The semantic equality behind `impl PartialEq for Value`. Total: values TLC refuses
/// to compare are simply unequal. Use [`crate::tlc::equals`] for TLC's checked
/// equality. Iterative over enumerated sets and functions.
pub(crate) fn semantic_eq(a: &Value, b: &Value) -> bool {
    if let Some(r) = scalar_eq(a, b) {
        return r;
    }
    let mut work: Vec<(&Value, &Value)> = vec![(a, b)];
    while let Some((x, y)) = work.pop() {
        let equal = match scalar_eq(x, y) {
            Some(r) => r,
            None => match (x, y) {
                (Value::Fcn(f), Value::Fcn(g)) => fcn_eq_step(f, g, &mut work),
                (Value::Set(s), Value::Set(t)) => set_eq_step(s, t, &mut work),
                _ => false,
            },
        };
        if !equal {
            return false;
        }
    }
    true
}

fn fcn_eq_step<'a>(f: &'a Fcn, g: &'a Fcn, work: &mut Vec<(&'a Value, &'a Value)>) -> bool {
    if f.len() != g.len() || f.kind() != g.kind() {
        return false;
    }
    match (&f.0.repr, &g.0.repr) {
        (FcnRepr::Tuple(a), FcnRepr::Tuple(b)) => work.extend(a.iter().zip(b.iter())),
        (FcnRepr::Record(a), FcnRepr::Record(b)) => {
            if f.order() == g.order() {
                for ((n, v), (m, w)) in a.iter().zip(b.iter()) {
                    if n != m {
                        return false;
                    }
                    work.push((v, w));
                }
            } else {
                for (n, v) in a {
                    match b.iter().find(|(m, _)| m == n) {
                        Some((_, w)) => work.push((v, w)),
                        None => return false,
                    }
                }
            }
        }
        (FcnRepr::General(a), FcnRepr::General(b)) => {
            if f.order() == g.order() {
                for ((d1, v1), (d2, v2)) in a.iter().zip(b.iter()) {
                    work.push((d1, d2));
                    work.push((v1, v2));
                }
            } else {
                for (d, v) in a {
                    match b.iter().find(|(e, _)| e == d) {
                        Some((_, w)) => work.push((v, w)),
                        None => return false,
                    }
                }
            }
        }
        _ => return false,
    }
    true
}

fn set_eq_step<'a>(s: &'a Set, t: &'a Set, work: &mut Vec<(&'a Value, &'a Value)>) -> bool {
    if s.ptr_eq(t) {
        return true;
    }
    if let (Some((l1, h1)), Some((l2, h2))) = (s.as_interval(), t.as_interval()) {
        return (h1 < l1 && h2 < l2) || (l1 == l2 && h1 == h2);
    }
    let (cs, ct) = (class(s), class(t));
    match (cs == Class::Structural, ct == Class::Structural) {
        (true, true) => return structural_eq(s, t),
        (true, false) | (false, true) => return false,
        (false, false) => {}
    }
    if quiet_size(s) != quiet_size(t) || s.hash64() != t.hash64() {
        return false;
    }
    if s.order() == t.order()
        && let (Some(a), Some(b)) = (materialized_ref(s), materialized_ref(t))
    {
        work.extend(a.iter().zip(b.iter()));
        return true;
    }
    let order = s.order();
    match (quiet_stream(s, order), quiet_stream(t, order)) {
        (Some(a), Some(b)) => a.zip(b).all(|(x, y)| semantic_eq(&x, &y)),
        _ => false,
    }
}

// ---------------------------------------------------------------------------------
// The total order
// ---------------------------------------------------------------------------------

fn rank(v: &Value) -> u8 {
    match v {
        Value::Model(_) => 0,
        Value::Bool(_) => 1,
        Value::Int(_) | Value::BigInt(_) => 2,
        Value::Str(_) => 3,
        Value::Set(s) => {
            if class(s) == Class::Structural {
                6
            } else {
                4
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
    /// 7. infinite, unenumerable or very large sets (`Nat < Int < STRING < Seq(S)`,
    ///    then by definition).
    ///
    /// Names (rules 1 and 4, and record fields) are ordered by [`ValueOrder`].
    /// Consistent with `==`: `a.cmp_with(b, o) == Equal` iff `a == b`.
    #[must_use]
    pub fn cmp_with(&self, other: &Self, order: ValueOrder) -> Ordering {
        total_cmp(self, other, order)
    }
}

/// After this many selections, [`for_each_sorted`] sorts what remains instead.
const SELECT_BEFORE_SORT: usize = 4;

/// Visits `elems` in ascending [`total_cmp`] order until `visit` decides,
/// without sorting when the decision comes early: a candidate set usually
/// differs from the set it is compared with in its first few elements.
/// `elems` must be pairwise distinct, so that the order is unique.
pub(crate) fn for_each_sorted<'a, T>(
    elems: &[&'a Value],
    order: ValueOrder,
    mut visit: impl FnMut(&'a Value) -> Result<Option<T>>,
) -> Result<Option<T>> {
    let mut rest: Vec<&'a Value> = elems.to_vec();
    let mut selected = 0usize;
    while !rest.is_empty() {
        if selected == SELECT_BEFORE_SORT {
            rest.sort_unstable_by(|a, b| total_cmp(a, b, order));
            for x in rest {
                if let Some(t) = visit(x)? {
                    return Ok(Some(t));
                }
            }
            return Ok(None);
        }
        let mut min = 0;
        for i in 1..rest.len() {
            if total_cmp(rest[i], rest[min], order) == Ordering::Less {
                min = i;
            }
        }
        if let Some(t) = visit(rest.swap_remove(min))? {
            return Ok(Some(t));
        }
        selected += 1;
    }
    Ok(None)
}

/// `Value::set_raw(elems).cmp_with(other, order)` for pairwise-distinct,
/// mutually comparable `elems` (see [`Value::cmp_distinct_set`]).
fn cmp_distinct_set(elems: &[&Value], other: &Value, order: ValueOrder) -> Option<Ordering> {
    let Value::Set(t) = other else {
        return None;
    };
    // The candidate is a small finite set (rank 4); only an enumerated `other`
    // of the same rank compares element by element.
    if !matches!(t.repr(), SetRepr::Enum | SetRepr::Raw(_)) || class(t) != Class::Finite {
        return None;
    }
    let n = u128::try_from(elems.len()).ok()?;
    if n > u128::from(MAX_ENUMERATION) {
        return None;
    }
    let c = n.cmp(&quiet_size(t)?);
    if c != Ordering::Equal {
        return Some(c);
    }
    let mut theirs = quiet_stream(t, order)?;
    let mut exhausted = false;
    let decided = for_each_sorted(elems, order, |x| {
        let Some(y) = theirs.next() else {
            exhausted = true;
            return Ok(Some(Ordering::Equal));
        };
        let c = total_cmp(x, &y, order);
        Ok((c != Ordering::Equal).then_some(c))
    })
    .ok()?;
    if exhausted {
        return None;
    }
    Some(decided.unwrap_or(Ordering::Equal))
}

impl Value {
    /// The order [`Value::cmp_with`] gives the set of `elems`, built raw,
    /// against `other`, computed without building or sorting that set: its
    /// elements are selected in order only up to the first difference.
    /// `None` when `other` is not a finite enumerated set, so that the caller
    /// must build the set and compare it in full.
    ///
    /// The caller must guarantee that `elems` are pairwise distinct and
    /// mutually comparable ([`Value::elements_comparable`]), as the image of a
    /// normalized set under an injective map that preserves comparability is.
    ///
    /// ```
    /// use dive_value::{Ctx, Value};
    /// let ctx = Ctx::default();
    /// let (one, two, three) = (Value::Int(1), Value::Int(2), Value::Int(3));
    /// let other = Value::set_raw(vec![one.clone(), three.clone()], ctx);
    /// let full = Value::set_raw(vec![two.clone(), one.clone()], ctx).cmp_with(&other, ctx.order);
    /// assert_eq!(Value::cmp_distinct_set(&[&two, &one], &other, ctx.order), Some(full));
    /// ```
    #[must_use]
    pub fn cmp_distinct_set(
        elems: &[&Value],
        other: &Value,
        order: ValueOrder,
    ) -> Option<Ordering> {
        cmp_distinct_set(elems, other, order)
    }

    /// Whether every pair of `elems` compares in TLC without error, so that a
    /// set of them normalizes by the total order alone.
    ///
    /// ```
    /// use dive_value::Value;
    /// assert!(Value::elements_comparable(&[Value::Int(1), Value::Int(2)]));
    /// assert!(!Value::elements_comparable(&[Value::Int(1), Value::Bool(true)]));
    /// ```
    #[must_use]
    pub fn elements_comparable(elems: &[Value]) -> bool {
        all_comparable(elems)
    }
}

/// A function's `(domain, value)` pairs sorted in `order`.
fn pairs_in(f: &Fcn, order: ValueOrder) -> Cow<'_, [(Value, Value)]> {
    match &f.0.repr {
        FcnRepr::General(p) if f.order() == order => Cow::Borrowed(p),
        _ => {
            let mut v: Vec<(Value, Value)> = f.iter().map(|(d, x)| (d, x.clone())).collect();
            if !matches!(f.0.repr, FcnRepr::Tuple(_)) && f.order() != order {
                v.sort_by(|a, b| total_cmp(&a.0, &b.0, order));
            }
            Cow::Owned(v)
        }
    }
}

/// The order of scalars (and of values of different scalar kinds), or `None` when a
/// compound value is involved.
fn scalar_cmp(a: &Value, b: &Value, order: ValueOrder) -> Option<Ordering> {
    match (a, b) {
        (Value::Bool(p), Value::Bool(q)) => Some(p.cmp(q)),
        (Value::Int(p), Value::Int(q)) => Some(p.cmp(q)),
        (Value::Str(p), Value::Str(q)) | (Value::Model(p), Value::Model(q)) => {
            Some(p.cmp_with(q, order))
        }
        (Value::Set(_) | Value::Fcn(_), _) | (_, Value::Set(_) | Value::Fcn(_)) => None,
        _ => {
            let (ra, rb) = (rank(a), rank(b));
            if ra == rb {
                Some(a.as_int().cmp(&b.as_int()))
            } else {
                Some(ra.cmp(&rb))
            }
        }
    }
}

pub(crate) fn total_cmp(a: &Value, b: &Value, order: ValueOrder) -> Ordering {
    if let Some(c) = scalar_cmp(a, b, order) {
        return c;
    }
    if let (Value::Fcn(f), Value::Fcn(g)) = (a, b)
        && let Some(c) = flat_fcn_cmp(f, g, order)
    {
        return c;
    }
    let mut work: Vec<(&Value, &Value)> = vec![(a, b)];
    while let Some((x, y)) = work.pop() {
        let c = cmp_step(x, y, order, &mut work);
        if c != Ordering::Equal {
            return c;
        }
    }
    Ordering::Equal
}

/// [`fcn_cmp_step`] for tuples, or records in `order`, whose deciding values
/// are scalars, without a work list; `None` when a compound value would have
/// to be compared.
fn flat_fcn_cmp(f: &Fcn, g: &Fcn, order: ValueOrder) -> Option<Ordering> {
    if f.ptr_eq(g) {
        return Some(Ordering::Equal);
    }
    let c = f.len().cmp(&g.len());
    if c != Ordering::Equal {
        return Some(c);
    }
    let values = |pairs: &mut dyn Iterator<Item = (&Value, &Value)>| {
        for (v, w) in pairs {
            let c = scalar_cmp(v, w, order)?;
            if c != Ordering::Equal {
                return Some(c);
            }
        }
        Some(Ordering::Equal)
    };
    match (&f.0.repr, &g.0.repr) {
        (FcnRepr::Tuple(a), FcnRepr::Tuple(b)) => values(&mut a.iter().zip(b.iter())),
        (FcnRepr::Record(a), FcnRepr::Record(b)) if f.order() == order && g.order() == order => {
            for ((n, _), (m, _)) in a.iter().zip(b.iter()) {
                let c = n.cmp_with(m, order);
                if c != Ordering::Equal {
                    return Some(c);
                }
            }
            values(&mut a.iter().zip(b.iter()).map(|((_, v), (_, w))| (v, w)))
        }
        _ => None,
    }
}

/// Compares the top level of `x` and `y`; pushes the pairs that decide a tie, so that
/// the first of them is popped next.
fn cmp_step<'a>(
    x: &'a Value,
    y: &'a Value,
    order: ValueOrder,
    work: &mut Vec<(&'a Value, &'a Value)>,
) -> Ordering {
    if let Some(c) = scalar_cmp(x, y, order) {
        return c;
    }
    let (ra, rb) = (rank(x), rank(y));
    if ra != rb {
        return ra.cmp(&rb);
    }
    match (x, y) {
        (Value::Fcn(f), Value::Fcn(g)) => fcn_cmp_step(f, g, order, work),
        (Value::Set(s), Value::Set(t)) => set_cmp_step(s, t, ra, order, work),
        _ => Ordering::Equal,
    }
}

fn fcn_cmp_step<'a>(
    f: &'a Fcn,
    g: &'a Fcn,
    order: ValueOrder,
    work: &mut Vec<(&'a Value, &'a Value)>,
) -> Ordering {
    if f.ptr_eq(g) {
        return Ordering::Equal;
    }
    let c = f.len().cmp(&g.len());
    if c != Ordering::Equal {
        return c;
    }
    let same_order = f.order() == order && g.order() == order;
    match (&f.0.repr, &g.0.repr) {
        // Same domain 1..n: only the values decide.
        (FcnRepr::Tuple(a), FcnRepr::Tuple(b)) => {
            work.extend(a.iter().zip(b.iter()).rev());
            Ordering::Equal
        }
        (FcnRepr::Record(a), FcnRepr::Record(b)) if same_order => {
            for ((n, _), (m, _)) in a.iter().zip(b.iter()) {
                let c = n.cmp_with(m, order);
                if c != Ordering::Equal {
                    return c;
                }
            }
            work.extend(a.iter().zip(b.iter()).rev().map(|((_, v), (_, w))| (v, w)));
            Ordering::Equal
        }
        (FcnRepr::General(a), FcnRepr::General(b)) if same_order => {
            work.extend(a.iter().zip(b.iter()).rev().map(|((_, v), (_, w))| (v, w)));
            work.extend(a.iter().zip(b.iter()).rev().map(|((d, _), (e, _))| (d, e)));
            Ordering::Equal
        }
        // Different domain shapes or orders: compare owned copies.
        _ => {
            let (p, q) = (pairs_in(f, order), pairs_in(g, order));
            for ((d1, _), (d2, _)) in p.iter().zip(q.iter()) {
                let c = total_cmp(d1, d2, order);
                if c != Ordering::Equal {
                    return c;
                }
            }
            for ((_, v1), (_, v2)) in p.iter().zip(q.iter()) {
                let c = total_cmp(v1, v2, order);
                if c != Ordering::Equal {
                    return c;
                }
            }
            Ordering::Equal
        }
    }
}

fn set_cmp_step<'a>(
    s: &'a Set,
    t: &'a Set,
    rank: u8,
    order: ValueOrder,
    work: &mut Vec<(&'a Value, &'a Value)>,
) -> Ordering {
    if s.ptr_eq(t) {
        return Ordering::Equal;
    }
    if rank == 6 {
        return structural_cmp(s, t, order);
    }
    let c = quiet_size(s).cmp(&quiet_size(t));
    if c != Ordering::Equal {
        return c;
    }
    if let (Some((l1, _)), Some((l2, _))) = (s.as_interval(), t.as_interval()) {
        if interval_empty(s) {
            return Ordering::Equal;
        }
        return l1.cmp(l2);
    }
    if s.order() == order
        && t.order() == order
        && let (Some(a), Some(b)) = (materialized_ref(s), materialized_ref(t))
    {
        work.extend(a.iter().zip(b.iter()).rev());
        return Ordering::Equal;
    }
    match (quiet_stream(s, order), quiet_stream(t, order)) {
        (Some(a), Some(b)) => {
            for (x, y) in a.zip(b) {
                let c = total_cmp(&x, &y, order);
                if c != Ordering::Equal {
                    return c;
                }
            }
            Ordering::Equal
        }
        _ => structural_cmp(s, t, order),
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
    /// Functions with string domains (records, literal or not), by field-name
    /// list: two of them with different names are told apart by their names
    /// alone, and two with the same names compare field by field. The names
    /// are in stored order (see [`join_fields`]).
    Rcd(Vec<(Vec<Atom>, Vec<Shape>)>),
    Conflict,
}

/// Joins record shapes: per name list, field by field.
fn join_rcd(mut acc: Vec<(Vec<Atom>, Vec<Shape>)>, other: Vec<(Vec<Atom>, Vec<Shape>)>) -> Shape {
    for (names, fields) in other {
        if !join_fields(&mut acc, &names, |n| n, fields.into_iter()) {
            return Shape::Conflict;
        }
    }
    Shape::Rcd(acc)
}

/// Joins one record's field shapes into `acc`, keyed by its field names in
/// stored order; `false` on a conflict. The same names stored in another
/// order (records built under different name orders) are a conflict, so the
/// key never pairs fields by position wrongly.
fn join_fields<'a, N>(
    acc: &mut Vec<(Vec<Atom>, Vec<Shape>)>,
    names: &'a [N],
    name: impl Fn(&'a N) -> &'a Atom,
    fields: impl Iterator<Item = Shape>,
) -> bool {
    let same =
        |n: &[Atom]| n.len() == names.len() && n.iter().zip(names).all(|(a, b)| a == name(b));
    if let Some((_, mine)) = acc.iter_mut().find(|(n, _)| same(n)) {
        for (field, theirs) in mine.iter_mut().zip(fields) {
            let joined = join(std::mem::replace(field, Shape::Bottom), theirs);
            if joined == Shape::Conflict {
                return false;
            }
            *field = joined;
        }
        return true;
    }
    if acc
        .iter()
        .any(|(n, _)| n.len() == names.len() && names.iter().all(|other| n.contains(name(other))))
    {
        return false;
    }
    let fields: Vec<Shape> = fields.collect();
    if fields.contains(&Shape::Conflict) {
        return false;
    }
    acc.push((names.iter().map(|n| name(n).clone()).collect(), fields));
    true
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
        (Shape::Rcd(a), Shape::Rcd(b)) => join_rcd(a, b),
        // Only the empty tuple compares with a record without error (TLC's
        // `toRcd` of `<<>>`, and `FcnRcdValue.compareTo` by size); any other
        // tuple or function fails in one of the two directions.
        (Shape::Rcd(r), Fcn(d, v)) | (Fcn(d, v), Shape::Rcd(r)) if *d == Bottom && *v == Bottom => {
            Shape::Rcd(r)
        }
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
        // Records join into the accumulated record shape in place, without
        // building a shape of their own.
        if let (Shape::Rcd(entries), Value::Fcn(f)) = (&mut acc, v)
            && let FcnRepr::Record(fs) = &f.0.repr
        {
            let Ok(_guard) = crate::depth::Guard::enter() else {
                return Shape::Conflict;
            };
            let joined = join_fields(entries, fs, |(n, _)| n, fs.iter().map(|(_, v)| shape(v)));
            if !joined {
                return Shape::Conflict;
            }
            continue;
        }
        acc = join(acc, shape(v));
        if acc == Shape::Conflict {
            break;
        }
    }
    acc
}

fn shape(v: &Value) -> Shape {
    // Very deep values simply take the exact (slow) path.
    let Ok(_guard) = crate::depth::Guard::enter() else {
        return Shape::Conflict;
    };
    match v {
        Value::Bool(_) => Shape::Bool,
        Value::Int(_) | Value::BigInt(_) => Shape::Int,
        Value::Str(_) => Shape::Str,
        Value::Model(a) => a.mv_type().map_or(Shape::AnyMv, Shape::TypedMv),
        Value::Set(s) => match s.repr() {
            SetRepr::Enum => match s.0.elems.get() {
                Some(e) => match join_all(e.iter()) {
                    Shape::Conflict => Shape::Conflict,
                    j => Shape::Set(Box::new(j)),
                },
                None => Shape::Conflict,
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
            FcnRepr::Record(fs) => {
                let mut acc = Vec::new();
                if join_fields(&mut acc, fs, |(n, _)| n, fs.iter().map(|(_, v)| shape(v))) {
                    Shape::Rcd(acc)
                } else {
                    Shape::Conflict
                }
            }
            // An empty explicit domain would pass for the empty tuple.
            FcnRepr::General(ps) if ps.is_empty() => Shape::Conflict,
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
        elems.sort_by(|a, b| total_cmp(a, b, ctx.order));
        elems.dedup_by(|later, earlier| total_cmp(later, earlier, ctx.order) == Ordering::Equal);
        return Ok(elems);
    }
    // Decide errors exactly as TLC does, in TLC's order; keep TLC's survivors.
    let mut out = tlc_insertion_sort(
        elems,
        Ctx {
            order: ValueOrder::Tlc,
            ..ctx
        },
    )?;
    if ctx.order != ValueOrder::Tlc {
        out.sort_by(|a, b| total_cmp(a, b, ctx.order));
    }
    Ok(out)
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
/// `FcnRcdValue.normalize` does. A string domain gives a record that is not a record
/// literal (TLC keeps such functions as `FcnRcdValue`).
pub(crate) fn make_fcn(mut pairs: Vec<(Value, Value)>, ctx: Ctx) -> Result<Value> {
    if pairs.is_empty() {
        return Ok(Value::tuple_fcn(Vec::new()));
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
            tlc_fcn_normalize(
                &mut pairs,
                Ctx {
                    order: ValueOrder::Tlc,
                    ..ctx
                },
            )?;
            if ctx.order != ValueOrder::Tlc {
                pairs.sort_by(|a, b| total_cmp(&a.0, &b.0, ctx.order));
            }
        }
    }
    // Canonical representation by domain.
    let n = pairs.len();
    let is_tuple = pairs.iter().enumerate().all(
        |(i, (d, _))| matches!(d, Value::Int(k) if i64::try_from(i).is_ok_and(|i| *k == i + 1)),
    );
    if is_tuple {
        return Ok(Value::tuple_fcn(
            pairs.into_iter().map(|(_, v)| v).collect(),
        ));
    }
    if pairs.iter().all(|(d, _)| matches!(d, Value::Str(_))) {
        let mut fields: Vec<(Atom, Value)> = Vec::with_capacity(n);
        for (d, v) in pairs {
            if let Value::Str(a) = d {
                fields.push((a, v));
            }
        }
        return Ok(Value::Fcn(Fcn::record(fields, ctx.order, false)));
    }
    Ok(Value::Fcn(Fcn::general(pairs, ctx.order)))
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

// ---------------------------------------------------------------------------------
// Function domains
// ---------------------------------------------------------------------------------

impl Fcn {
    /// Whether the function's domain, as a set, equals `dom` (`DOMAIN f = dom`
    /// under `==`), decided without building the domain set: by the bounds of
    /// an interval, by lookup in an enumerated set, or factor by factor for a
    /// product of those. `None` when no such shortcut applies, or when
    /// building the domain set could fail (domain elements TLC cannot
    /// compare); the caller then builds the set.
    ///
    /// ```
    /// use dive_value::{Ctx, Int, Value};
    /// let ctx = Ctx::default();
    /// let (Value::Fcn(f), Value::Set(d)) = (
    ///     Value::tuple(vec![Value::TRUE, Value::FALSE]),
    ///     Value::interval(Int::Small(1), Int::Small(2), ctx),
    /// ) else {
    ///     unreachable!()
    /// };
    /// assert_eq!(f.domain_equals(&d), Some(true));
    /// ```
    #[must_use]
    pub fn domain_equals(&self, dom: &Set) -> Option<bool> {
        if !self.domain_comparable() {
            return None;
        }
        let n = self.len() as u128;
        match dom.repr() {
            SetRepr::Interval(..) | SetRepr::Enum => {
                if quiet_size(dom)? != n {
                    return Some(false);
                }
                let mut verdict = Some(true);
                self.for_each_key(&mut |k| {
                    if verdict == Some(true) {
                        verdict = contains_settled(dom, k);
                    }
                });
                verdict
            }
            SetRepr::Product(factors) => {
                // A product too large to enumerate compares by structure.
                if class(dom) == Class::Structural {
                    return None;
                }
                let mut size: u128 = 1;
                for f in factors {
                    let Value::Set(f) = f else { return None };
                    if !matches!(f.repr(), SetRepr::Interval(..) | SetRepr::Enum) {
                        return None;
                    }
                    size = size.checked_mul(quiet_size(f)?)?;
                }
                if size != n {
                    return Some(false);
                }
                let mut verdict = Some(true);
                self.for_each_key(&mut |k| {
                    if verdict == Some(true) {
                        verdict = in_product(factors, k);
                    }
                });
                verdict
            }
            _ => None,
        }
    }

    /// Whether building the set of the domain elements cannot fail: as in
    /// `normalize_set`, every pair is comparable in TLC.
    fn domain_comparable(&self) -> bool {
        match &self.0.repr {
            // The integers `1..n`, or strings.
            FcnRepr::Tuple(_) | FcnRepr::Record(_) => true,
            FcnRepr::General(p) => {
                p.len() <= 1
                    || p.iter()
                        .all(|(d, _)| matches!(d, Value::Int(_) | Value::BigInt(_)))
                    || join_all(p.iter().map(|(d, _)| d)) != Shape::Conflict
            }
        }
    }

    /// Visits the domain elements, without cloning stored ones.
    fn for_each_key(&self, visit: &mut dyn FnMut(&Value)) {
        match &self.0.repr {
            FcnRepr::Tuple(e) => {
                for i in 1..=e.len() {
                    visit(&Value::Int(i64::try_from(i).unwrap_or(i64::MAX)));
                }
            }
            FcnRepr::Record(f) => {
                for (a, _) in f {
                    visit(&Value::Str(a.clone()));
                }
            }
            FcnRepr::General(p) => {
                for (d, _) in p {
                    visit(d);
                }
            }
        }
    }
}

/// Whether `k` is a tuple whose components are members of `factors` in turn,
/// or `None` when a factor cannot decide without enumeration.
fn in_product(factors: &[Value], k: &Value) -> Option<bool> {
    let Some(t) = k.as_fcn().and_then(Fcn::as_tuple) else {
        return Some(false);
    };
    if t.len() != factors.len() {
        return Some(false);
    }
    for (x, f) in t.iter().zip(factors) {
        if !contains_settled(f.as_set()?, x)? {
            return Some(false);
        }
    }
    Some(true)
}

/// `x \in s` under `==` for an interval or an enumerated set whose elements
/// are stored, or `None` for other sets and big integers.
fn contains_settled(s: &Set, x: &Value) -> Option<bool> {
    match s.repr() {
        SetRepr::Interval(lo, hi) => {
            let (lo, hi) = (lo.to_i64()?, hi.to_i64()?);
            match x {
                Value::Int(v) => Some(lo <= *v && *v <= hi),
                Value::BigInt(_) => None,
                _ => Some(false),
            }
        }
        SetRepr::Enum => {
            let elems = s.stored_elements()?;
            let order = s.order();
            Some(elems.binary_search_by(|e| total_cmp(e, x, order)).is_ok())
        }
        _ => None,
    }
}
