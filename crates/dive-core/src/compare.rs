//! Equality, membership and set-construction rules of the default profile
//! (`02-semantic-contract.md` C1–C3, D-009), as the reference engine applies
//! them (`crates/dive-eval/src/tla_cmp.rs`).
//!
//! - Two values of the same class (boolean, integer, string, set, function)
//!   compare extensionally; inside sets and functions, values of different
//!   classes are unequal.
//! - A direct comparison of values of different classes (`1 = "a"`,
//!   `{1} = 1`, `<<1>> = {1}`) is TLC's error.
//! - Membership that fails to find the element raises TLC's error when the
//!   set holds a scalar of another class than the element: TLC compares them
//!   on its way and fails.
//! - The core builds every set normalized, but TLC normalizes lazily, and
//!   whether and which error a set mixing incomparable values raises depends
//!   on when TLC normalizes it (C1, C2). The core refuses to build such a set
//!   rather than guess.
//!
//! Model values follow TLC's `ModelValue` (`crates/dive-value/src/tlc.rs`):
//! - an untyped model value is unequal to every other value, without error,
//!   and is never a member of a set of another kind;
//! - a typed model value (`A_x`: more than two characters, the second `_`)
//!   compares only with model values of its own type or untyped ones; any
//!   other comparison is TLC's error.

use crate::Error;
use crate::handle::{Handle, Tag};
use crate::heap::Heap;
use crate::node::Kind;

/// The comparison class of a value.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Class {
    Model,
    Bool,
    Int,
    Str,
    Set,
    Fcn,
}

impl Class {
    const fn is_scalar(self) -> bool {
        matches!(self, Self::Bool | Self::Int | Self::Str)
    }

    const fn noun(self) -> &'static str {
        match self {
            Self::Model => "model value",
            Self::Bool => "boolean",
            Self::Int => "integer",
            Self::Str => "string",
            Self::Set => "set",
            Self::Fcn => "function",
        }
    }

    /// The noun with its indefinite article.
    const fn a(self) -> &'static str {
        match self {
            Self::Model => "a model value",
            Self::Bool => "a boolean",
            Self::Int => "an integer",
            Self::Str => "a string",
            Self::Set => "a set",
            Self::Fcn => "a function",
        }
    }
}

/// The class of a handle.
pub(crate) fn class(heap: &Heap, h: Handle) -> Class {
    match h.tag() {
        Tag::Bool => Class::Bool,
        Tag::Int => Class::Int,
        Tag::Str => Class::Str,
        Tag::Model => Class::Model,
        Tag::Persistent | Tag::Transient | Tag::Empty => {
            if !h.is_compound() {
                return Class::Set;
            }
            match heap.view(h).kind {
                Kind::BigInt => Class::Int,
                k if k.is_fcn() => Class::Fcn,
                _ => Class::Set,
            }
        }
    }
}

fn mismatch(a: Class, b: Class) -> Error {
    // `Assert.fail(EC.GENERAL)` in the values' `equals`/`compareTo`.
    Error::eval_class(
        crate::TlcClass::GENERAL,
        format!(
            "Attempted to check equality of {} with a non-{} value: {}",
            a.a(),
            a.noun(),
            b.a()
        ),
    )
}

/// TLC's type of a model value: the first character of a name of more than
/// two UTF-16 units whose second unit is `_` (`ModelValue`'s constructor).
/// `None` for an untyped model value or a value that is not one.
pub(crate) fn mv_type(heap: &Heap, h: Handle) -> Option<u16> {
    let text = heap.store().strings().text(h.as_model_id()?)?;
    let mut units = text.encode_utf16();
    let (first, second) = (units.next()?, units.next()?);
    (second == u16::from(b'_') && units.next().is_some()).then_some(first)
}

/// Whether TLC can compare `a` and `b` when either is a model value
/// (`ModelValue.compareTo`/`equals`): `None` when neither is one.
fn mv_comparable(heap: &Heap, a: Handle, b: Handle) -> Option<bool> {
    let (ma, mb) = (a.as_model_id().is_some(), b.as_model_id().is_some());
    if !ma && !mb {
        return None;
    }
    let (ta, tb) = (mv_type(heap, a), mv_type(heap, b));
    Some(match (ma, mb) {
        (true, true) => ta.is_none() || tb.is_none() || ta == tb,
        (true, false) => ta.is_none(),
        _ => tb.is_none(),
    })
}

fn mv_error(heap: &Heap, a: Handle, b: Handle) -> Error {
    let both = a.as_model_id().is_some() && b.as_model_id().is_some();
    let name = |h: Handle| {
        h.as_model_id()
            .and_then(|id| heap.store().strings().text(id))
            .map_or_else(String::new, |t| t.to_string())
    };
    let text = if both {
        format!(
            "Attempted to check equality of the differently-typed model values {} and {}",
            name(a),
            name(b)
        )
    } else {
        let m = if a.as_model_id().is_some() { a } else { b };
        format!(
            "Attempted to check equality of typed model value {} and non-model value",
            name(m)
        )
    };
    Error::eval_class(crate::TlcClass::GENERAL, text)
}

/// `a = b` on normalized handles.
///
/// # Errors
/// TLC's error for values of different classes.
pub(crate) fn equals(heap: &Heap, a: Handle, b: Handle) -> Result<bool, Error> {
    if a == b {
        return Ok(true);
    }
    match mv_comparable(heap, a, b) {
        Some(true) => return Ok(false),
        Some(false) => return Err(mv_error(heap, a, b)),
        None => {}
    }
    let (ca, cb) = (class(heap, a), class(heap, b));
    if ca != cb {
        return Err(mismatch(ca, cb));
    }
    let eq = heap.eq(a, b);
    if !eq && !ca.is_scalar() {
        guard_equals(heap, a, b)?;
    }
    Ok(eq)
}

/// After a miss of `x` in the enumerated set with sorted elements `elems`:
/// TLC's error if it would have compared `x` with a scalar of another class.
///
/// Elements are sorted by class (booleans, integers, strings, then
/// compounds), so the first and last elements decide.
pub(crate) fn miss(heap: &Heap, x: Handle, elems: &[Handle]) -> Result<(), Error> {
    guard_member(heap, x, elems)?;
    miss_scalars(heap, x, elems)
}

/// A demanded explicit-set miss. A certificate replaces the complete shape
/// guard with its exact witness, but never the scalar/model-value checks.
pub(crate) fn miss_cached(
    heap: &Heap,
    memo: &mut crate::memo::Memo,
    x: Handle,
    set: Handle,
    elems: &[Handle],
) -> Result<(), Error> {
    if !memo.members_active()
        || !heap.store().semantics().tlc_comparisons
        || !x.is_compound()
        || !set.is_persistent()
        || heap.view(set).kind != Kind::Set
        || elems.is_empty()
    {
        return miss(heap, x, elems);
    }
    if let Some(witness) = memo.member_witness(set) {
        // A failed proof always takes the original full path. In particular,
        // no cached shape certificate changes the refusal or its demand site.
        if guard_member(heap, x, &[witness]).is_ok() {
            return miss_scalars(heap, x, elems);
        }
        return miss(heap, x, elems);
    }
    // Admission happens only after the demanded original miss succeeds.
    miss(heap, x, elems)?;
    if let Some(witness) = member_witness(heap, elems) {
        memo.remember_member(set, witness);
    }
    Ok(())
}

fn miss_scalars(heap: &Heap, x: Handle, elems: &[Handle]) -> Result<(), Error> {
    // Model values sort first. Which of them TLC compares `x` with depends
    // on its search, so any incomparable pair is refused (C1, D3).
    let k = elems.partition_point(|e| e.as_model_id().is_some());
    let (mvs, elems) = elems.split_at(k);
    if x.as_model_id().is_some() {
        if mv_type(heap, x).is_none() {
            return Ok(());
        }
        if mvs
            .iter()
            .any(|&m| mv_comparable(heap, x, m) == Some(false))
            || !elems.is_empty()
        {
            return Err(Error::unsupported(
                "a membership test of a typed model value in a set holding values it cannot be compared with (C1, D3)",
            ));
        }
        return Ok(());
    }
    if mvs.iter().any(|&m| mv_type(heap, m).is_some()) {
        return Err(Error::unsupported(
            "a membership test in a set holding a typed model value (C1, D3)",
        ));
    }
    let (Some(&first), Some(&last)) = (elems.first(), elems.last()) else {
        return Ok(());
    };
    let cx = class(heap, x);
    let (cf, cl) = (class(heap, first), class(heap, last));
    let clash = if cx.is_scalar() {
        if cf == cx {
            (cl != cx).then_some(cl)
        } else {
            Some(cf)
        }
    } else {
        cf.is_scalar().then_some(cf)
    };
    match clash {
        Some(c) => Err(mismatch(cx, c)),
        None => Ok(()),
    }
}

/// Whether a function is record-like (string keys) rather than tuple-like
/// (an integer domain), for TLC's asymmetric record/tuple order (C2, C5).
fn record_like(heap: &Heap, f: Handle) -> bool {
    let v = heap.view(f);
    v.kind == Kind::Fcn && v.keys().first().is_some_and(|k| k.as_str_id().is_some())
}

/// Refuses a set, given its sorted elements, whose elements TLC cannot all
/// compare: values of different classes, records with tuples, or compound
/// elements whose nested comparisons are not proved safe.
///
/// # Errors
/// [`crate::ErrorKind::Unsupported`], naming the mixture.
pub(crate) fn check_set(heap: &Heap, elems: &[Handle]) -> Result<(), Error> {
    // Model values sort first. Untyped ones compare with everything; a
    // typed one only with model values of its type or untyped ones.
    let k = elems.partition_point(|e| e.as_model_id().is_some());
    let (mvs, elems) = elems.split_at(k);
    let mut ty = None;
    for &m in mvs {
        if let Some(t) = mv_type(heap, m) {
            if !elems.is_empty() || ty.is_some_and(|u| u != t) {
                return Err(Error::unsupported(
                    "a set mixing a typed model value with values it cannot be compared with: whether and how TLC fails on it depends on when it normalizes the set (C1, C2)",
                ));
            }
            ty = Some(t);
        }
    }
    let (Some(&first), Some(&last)) = (elems.first(), elems.last()) else {
        return Ok(());
    };
    if elems.len() < 2 {
        return Ok(());
    }
    // Model values are holes or refused above.
    let tlc = heap.store().semantics().tlc_comparisons;
    if tlc {
        guard_set(heap, elems)?;
    }
    let (cf, cl) = (class(heap, first), class(heap, last));
    let refuse = |a: Class, b: Class| {
        Err(Error::unsupported(format!(
            "a set mixing {} and {}: whether and how TLC fails on it depends on when it normalizes the set (C1, C2)",
            a.a(),
            b.a()
        )))
    };
    if cf != cl {
        return refuse(cf, cl);
    }
    match cf {
        // Finite sets sort before functions and infinite sets after them.
        Class::Set if heap.view(last).kind.is_symbolic() => {
            if elems.iter().any(|&e| class(heap, e) == Class::Fcn) {
                return refuse(Class::Set, Class::Fcn);
            }
            Ok(())
        }
        Class::Fcn => {
            let rec = record_like(heap, first);
            if elems.iter().any(|&e| record_like(heap, e) != rec) {
                return Err(Error::unsupported(
                    "a set mixing records and other functions: TLC's order between them is asymmetric (C2, C5)",
                ));
            }
            Ok(())
        }
        _ => Ok(()),
    }?;
    // D-009 changes equality and membership, not normalization (C2). Keep
    // the default profile's existing class/record refusals above, then prove
    // nested comparisons safe. Scalar sets need no recursive shape walk.
    if !tlc && !cf.is_scalar() {
        guard_set(heap, elems)?;
    }
    Ok(())
}

// ----------------------------------------------------------------------
// Comparison shape guards (set normalization applies in both profiles)
// ----------------------------------------------------------------------

/// The comparison shape of a value under TLC's comparisons: what any
/// `equals` or `compareTo` TLC runs on it could inspect. Two values whose
/// shapes unify cannot make TLC fail, in any order of comparison, and then
/// TLC's answers are TLA+'s, which the core computes. Where they do not
/// unify, TLC may fail, or answer by the asymmetric rules of C2 and C5, and
/// the core refuses (`02-semantic-contract.md` C1–C5) until TLC's checked
/// comparisons are ported.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Shape {
    /// No constraint: an untyped model value, or the element of an empty
    /// set.
    Hole,
    Bool,
    Int,
    Str,
    /// A typed model value of the given type.
    Typed(u16),
    /// A finite set, with its elements' unified shape.
    Set(Box<Shape>),
    /// The empty function (`<<>>`).
    EmptyFcn,
    /// A function over strings (a record), by field.
    Rec(std::collections::BTreeMap<u32, Shape>),
    /// A function over integers (a tuple or sequence), by index.
    IntKeyed(std::collections::BTreeMap<i64, Shape>),
    /// Any other function: its keys' and its values' unified shapes.
    Gen(Box<Shape>, Box<Shape>),
}

/// The shape of `h`, or `None` for a value whose comparisons the guard
/// does not model (a symbolic set or a lazy value).
fn shape(heap: &Heap, h: Handle) -> Option<Shape> {
    match h.tag() {
        Tag::Bool => return Some(Shape::Bool),
        Tag::Int => return Some(Shape::Int),
        Tag::Str => return Some(Shape::Str),
        Tag::Model => return Some(mv_type(heap, h).map_or(Shape::Hole, Shape::Typed)),
        Tag::Persistent | Tag::Transient | Tag::Empty => {}
    }
    if !h.is_compound() {
        return Some(Shape::Set(Box::new(Shape::Hole)));
    }
    let v = heap.view(h);
    match v.kind {
        Kind::BigInt => Some(Shape::Int),
        Kind::Set => {
            let mut acc = Shape::Hole;
            for &e in v.data {
                acc = unify(acc, shape(heap, e)?)?;
            }
            Some(Shape::Set(Box::new(acc)))
        }
        Kind::Interval => Some(Shape::Set(Box::new(Shape::Int))),
        Kind::IntFcn if v.data.is_empty() => Some(Shape::EmptyFcn),
        Kind::IntFcn => {
            let mut m = std::collections::BTreeMap::new();
            for (i, &e) in (v.lo..).zip(v.data) {
                m.insert(i, shape(heap, e)?);
            }
            Some(Shape::IntKeyed(m))
        }
        Kind::Fcn => {
            let (keys, vals) = (v.keys(), v.values());
            if keys.is_empty() {
                return Some(Shape::EmptyFcn);
            }
            if keys.iter().all(|k| k.as_str_id().is_some()) {
                let mut m = std::collections::BTreeMap::new();
                for (k, &e) in keys.iter().zip(vals) {
                    m.insert(k.as_str_id()?, shape(heap, e)?);
                }
                return Some(Shape::Rec(m));
            }
            if keys.iter().all(|k| k.as_int().is_some()) {
                let mut m = std::collections::BTreeMap::new();
                for (k, &e) in keys.iter().zip(vals) {
                    m.insert(k.as_int()?, shape(heap, e)?);
                }
                return Some(Shape::IntKeyed(m));
            }
            let (mut ks, mut vs) = (Shape::Hole, Shape::Hole);
            for (&k, &e) in keys.iter().zip(vals) {
                ks = unify(ks, shape(heap, k)?)?;
                vs = unify(vs, shape(heap, e)?)?;
            }
            Some(Shape::Gen(Box::new(ks), Box::new(vs)))
        }
        _ => None,
    }
}

fn unify_maps<K: Ord>(
    mut a: std::collections::BTreeMap<K, Shape>,
    b: std::collections::BTreeMap<K, Shape>,
) -> Option<std::collections::BTreeMap<K, Shape>> {
    for (k, y) in b {
        let x = a.remove(&k).unwrap_or(Shape::Hole);
        a.insert(k, unify(x, y)?);
    }
    Some(a)
}

/// The most specific shape compatible with both, or `None` when TLC could
/// fail comparing values of the two shapes.
fn unify(a: Shape, b: Shape) -> Option<Shape> {
    use Shape as S;
    Some(match (a, b) {
        // `<<>>` against a record is C2/C5's asymmetric case, so the empty
        // function unifies with the other non-record functions only.
        (S::Hole, x)
        | (x, S::Hole)
        | (S::EmptyFcn, x @ (S::IntKeyed(_) | S::Gen(..)))
        | (x @ (S::IntKeyed(_) | S::Gen(..)), S::EmptyFcn) => x,
        (S::Bool, S::Bool) => S::Bool,
        (S::Int, S::Int) => S::Int,
        (S::Str, S::Str) => S::Str,
        (S::Typed(x), S::Typed(y)) if x == y => S::Typed(x),
        (S::Set(x), S::Set(y)) => S::Set(Box::new(unify(*x, *y)?)),
        (S::EmptyFcn, S::EmptyFcn) => S::EmptyFcn,
        (S::Rec(x), S::Rec(y)) => S::Rec(unify_maps(x, y)?),
        (S::IntKeyed(x), S::IntKeyed(y)) => S::IntKeyed(unify_maps(x, y)?),
        (S::Gen(k1, v1), S::Gen(k2, v2)) => {
            S::Gen(Box::new(unify(*k1, *k2)?), Box::new(unify(*v1, *v2)?))
        }
        _ => return None,
    })
}

/// Finds a persistent member whose exact shape is the join of all members.
/// The partial join is associative: holes add no constraint, function maps
/// join independently at each key, and incompatible constructors stay
/// incompatible. Thus joining a query with this witness succeeds exactly
/// when joining the query with every member succeeds. Merely compatible
/// witnesses are insufficient: an empty function, for example, must not
/// represent a collection containing nonempty tuples. The equality below
/// checks the complete recursive shape, never a hash or a value kind.
fn member_witness(heap: &Heap, elems: &[Handle]) -> Option<Handle> {
    let aggregate = elems
        .iter()
        .try_fold(Shape::Hole, |acc, &e| unify(acc, shape(heap, e)?))?;
    elems.iter().copied().find(|&e| {
        e.is_persistent() && shape(heap, e).is_some_and(|candidate| candidate == aggregate)
    })
}

fn guard_refusal(what: &str) -> Error {
    Error::unsupported(format!(
        "{what} under --compat=tlc, where TLC's checked comparison may fail or answer by its asymmetric rules (C1–C5; the exact port is not built)"
    ))
}

/// Under TLC's comparisons, refuses `a = b` on two compound values unless
/// TLC cannot fail on them. The caller has handled identical handles,
/// model values and different classes.
fn guard_equals(heap: &Heap, a: Handle, b: Handle) -> Result<(), Error> {
    if !heap.store().semantics().tlc_comparisons {
        return Ok(());
    }
    let ok = match (shape(heap, a), shape(heap, b)) {
        (Some(x), Some(y)) => unify(x, y).is_some(),
        _ => false,
    };
    if ok {
        Ok(())
    } else {
        Err(guard_refusal(
            "an equality between values of different shapes",
        ))
    }
}

/// Under TLC's comparisons, refuses a membership miss of the compound `x`
/// among `elems` (a set's elements or a function's keys) unless TLC cannot
/// fail comparing `x` with any of them.
fn guard_member(heap: &Heap, x: Handle, elems: &[Handle]) -> Result<(), Error> {
    if !heap.store().semantics().tlc_comparisons || !x.is_compound() || elems.is_empty() {
        return Ok(());
    }
    let ok = (|| {
        let mut acc = shape(heap, x)?;
        for &e in elems {
            acc = unify(acc, shape(heap, e)?)?;
        }
        Some(acc)
    })()
    .is_some();
    if ok {
        Ok(())
    } else {
        Err(guard_refusal(
            "a membership test of a value of another shape",
        ))
    }
}

/// In either profile, refuses a set whose elements TLC could fail to sort
/// (C2): their shapes must unify.
fn guard_set(heap: &Heap, elems: &[Handle]) -> Result<(), Error> {
    if elems.len() < 2 {
        return Ok(());
    }
    let tlc = heap.store().semantics().tlc_comparisons;
    let ok = (|| {
        let mut acc = Shape::Hole;
        for &e in elems {
            acc = unify(acc, shape(heap, e)?)?;
        }
        Some(acc)
    })()
    .is_some();
    if ok {
        Ok(())
    } else if tlc {
        Err(guard_refusal("a set of values of different shapes"))
    } else {
        Err(Error::unsupported(
            "a set of values of different shapes: TLC's checked normalization may fail (C1, C2; the exact comparison order is not built)",
        ))
    }
}

/// Under TLC's comparisons, refuses an answer the default profile gives
/// by D-009 where TLC's own `member` may fail instead (C3).
pub(crate) fn guard_kind_member(heap: &Heap, what: &str) -> Result<(), Error> {
    if heap.store().semantics().tlc_comparisons {
        return Err(guard_refusal(what));
    }
    Ok(())
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

    fn heap() -> Heap {
        Heap::new(Arc::new(Store::default()), false, false)
    }

    fn shape_values(h: &mut Heap) -> Vec<Handle> {
        let one = Handle::int(1).unwrap();
        let three = Handle::int(3).unwrap();
        let a = h.string("a").unwrap();
        let b = h.string("b").unwrap();
        let mv = |name| Handle::model(h.store().strings().intern(name).unwrap());
        let mut values = vec![
            Handle::TRUE,
            one,
            a,
            mv("u"),
            mv("t_a"),
            mv("t_b"),
            mv("s_c"),
            h.int_from_i64(i64::MAX),
            h.tuple(&[]),
            h.tuple(&[one]),
            h.tuple(&[one, a]),
            h.int_fcn(-2, &[one, a]),
            h.fcn_sorted(&[one, three], &[one, a]),
            h.record(&[a], &[one]),
            h.record(&[a, b], &[one, a]),
            h.fcn_sorted(&[Handle::FALSE, Handle::TRUE], &[one, one]),
            h.fcn_sorted(&[one, a], &[one, one]),
            h.set_from(&mut vec![]),
            h.set_from(&mut vec![one, a]),
            h.interval(1, 3),
            h.set_enum(&[one, three]),
            h.symbolic(Kind::Nat, 0, &[]),
            h.symbolic(Kind::SeqSet, 0, &[one]),
        ];
        let previous = values.clone();
        for v in previous {
            values.push(h.tuple(&[v]));
            values.push(h.record(&[a], &[v]));
            values.push(h.set_from(&mut vec![v]));
        }
        values
    }

    #[test]
    fn shape_join_regrouping_preserves_success_and_complete_constraints() {
        let mut h = heap();
        let shapes: Vec<_> = shape_values(&mut h)
            .iter()
            .filter_map(|&v| shape(&h, v))
            .collect();
        for a in &shapes {
            for b in &shapes {
                for c in &shapes {
                    let left = unify(a.clone(), b.clone()).and_then(|ab| unify(ab, c.clone()));
                    let right = unify(b.clone(), c.clone()).and_then(|bc| unify(a.clone(), bc));
                    assert_eq!(left, right, "{a:?}; {b:?}; {c:?}");
                }
            }
        }
    }

    #[test]
    fn exact_witnesses_handle_genesis_positions_and_reject_partial_constraints() {
        let store = Arc::new(Store::with_semantics(true, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let one = Handle::int(1).unwrap();
        let u = Handle::model(h.store().strings().intern("u").unwrap());
        let genesis = h.tuple(&[]);
        let genesis = h.intern_exact(genesis).unwrap();
        let vertex = h.tuple(&[u, one]);
        let vertex = h.intern_exact(vertex).unwrap();
        for elems in [[genesis, vertex], [vertex, genesis]] {
            assert_eq!(member_witness(&h, &elems), Some(vertex));
        }
        assert_eq!(
            member_witness(&h, &[genesis, vertex, genesis]),
            Some(vertex)
        );
        let left = h.tuple(&[one, u]);
        let left = h.intern_exact(left).unwrap();
        let right = h.tuple(&[u, one]);
        let right = h.intern_exact(right).unwrap();
        assert_eq!(member_witness(&h, &[left, right]), None);
        let query = h.tuple(&[one, Handle::TRUE]);
        assert!(guard_member(&h, query, &[right]).is_err());
        assert!(guard_member(&h, query, &[left]).is_ok());
        assert!(guard_member(&h, query, &[left, right]).is_err());
        // A kind-only or merely compatible witness would hide the failure.
        assert_eq!(h.view(left).kind, h.view(right).kind);
        assert_eq!(member_witness(&h, &[genesis, left, right]), None);
    }

    #[test]
    fn cold_and_warm_membership_certificates_match_the_full_miss() {
        for hashcons in [true, false] {
            let store = Arc::new(Store::with_semantics(hashcons, crate::Semantics::tlc()));
            let mut h = Heap::new(store, false, false);
            let mut probes = shape_values(&mut h);
            let exact: Vec<_> = probes.iter().map(|&v| h.intern_exact(v).unwrap()).collect();
            probes.extend(exact.iter().copied());
            let mut memo = crate::memo::Memo::new(&[]);
            memo.set_cap(512);
            memo.reserve_members(true);
            let mut admitted = 0;
            for &a in &exact {
                for &b in &exact {
                    let set = h.set_from(&mut vec![a, b]);
                    let set = h.intern_exact(set).unwrap();
                    let elems = h.view(set).data;
                    for &x in &probes {
                        memo.clear();
                        let expected = miss(&h, x, elems);
                        assert_eq!(miss_cached(&h, &mut memo, x, set, elems), expected);
                        assert_eq!(miss_cached(&h, &mut memo, x, set, elems), expected);
                        admitted += usize::from(memo.member_witness(set).is_some());
                    }
                }
            }
            assert!(admitted > 0, "exercise hits, not only the fallback");
            assert!(!h.poisoned());
        }
    }

    #[test]
    fn lazy_queries_preserve_cold_refusals_and_failed_warm_proofs() {
        let store = Arc::new(Store::with_semantics(true, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        let vertex = h.tuple(&[one]);
        let set = h.set_from(&mut vec![vertex]);
        let set = h.intern_exact(set).unwrap();
        let domain = h.interval(1, 2);
        let lambda = h.lambda(0, domain, &[]);
        let predicate = h.set_pred(0, domain, &[]);
        let persistent_lambda = h.intern_const(lambda).unwrap();
        let persistent_predicate = h.intern_const(predicate).unwrap();
        for query in [lambda, predicate, persistent_lambda, persistent_predicate] {
            let mut memo = crate::memo::Memo::new(&[]);
            memo.reserve_members(true);
            let elems = h.view(set).data;
            let expected = miss(&h, query, elems);
            assert!(expected.is_err());
            assert_eq!(miss_cached(&h, &mut memo, query, set, elems), expected);
            assert_eq!(memo.bytes(), 0, "a refusal does not admit a certificate");
            let compatible = h.tuple(&[two]);
            let elems = h.view(set).data;
            assert_eq!(miss_cached(&h, &mut memo, compatible, set, elems), Ok(()));
            assert!(memo.member_witness(set).is_some());
            assert_eq!(miss_cached(&h, &mut memo, query, set, elems), expected);
        }
        assert!(!h.poisoned(), "comparison proofs do not force lazy values");
    }

    #[test]
    fn model_values_follow_tlcs_rules() {
        let h = heap();
        let mv = |t: &str| Handle::model(h.store().strings().intern(t).unwrap());
        let (u, v, ta, tb, sb) = (mv("u"), mv("v"), mv("t_a"), mv("t_b"), mv("s_b"));
        let one = Handle::int(1).unwrap();
        assert_eq!(mv_type(&h, u), None);
        assert_eq!(mv_type(&h, ta), Some(u16::from(b't')));
        assert_eq!(mv_type(&h, mv("t_")), None, "two units are untyped");
        assert!(!equals(&h, u, v).unwrap());
        assert!(!equals(&h, u, one).unwrap(), "untyped: unequal, no error");
        assert!(!equals(&h, one, u).unwrap());
        assert!(!equals(&h, ta, tb).unwrap());
        assert!(!equals(&h, ta, u).unwrap());
        assert!(equals(&h, ta, one).is_err());
        assert!(equals(&h, ta, sb).is_err(), "different types");
        assert!(check_set(&h, &[u, one]).is_ok());
        assert!(check_set(&h, &[ta, one]).is_err());
        assert!(check_set(&h, &[sb, ta]).is_err());
        assert!(miss(&h, u, &[one]).is_ok());
        assert!(miss(&h, one, &[u]).is_ok());
        assert!(miss(&h, ta, &[one]).is_err());
        assert!(h.cmp(u, Handle::FALSE).is_lt(), "model values sort first");
    }

    #[test]
    fn different_classes_are_an_error_and_same_classes_compare() {
        let mut h = heap();
        let one = Handle::int(1).unwrap();
        let a = h.string("a").unwrap();
        let s = h.set_from(&mut vec![one]);
        let t = h.tuple(&[one]);
        assert!(equals(&h, one, a).is_err());
        assert!(equals(&h, s, one).is_err());
        assert!(equals(&h, t, s).is_err());
        assert_eq!(
            equals(&h, one, a).unwrap_err().tlc(),
            Some(crate::TlcClass::GENERAL)
        );
        assert!(equals(&h, Handle::TRUE, one).is_err());
        assert!(equals(&h, one, one).unwrap());
        let sa = h.set_from(&mut vec![a]);
        assert!(!equals(&h, s, sa).unwrap(), "nested classes are unequal");
    }

    #[test]
    fn a_miss_against_another_scalar_class_is_an_error() {
        let mut h = heap();
        let (one, two) = (Handle::int(1).unwrap(), Handle::int(2).unwrap());
        let a = h.string("a").unwrap();
        let s = h.set_from(&mut vec![one]);
        assert!(miss(&h, a, &[one, two]).is_err());
        assert!(miss(&h, s, &[one, two]).is_err());
        assert!(miss(&h, Handle::int(3).unwrap(), &[one, two]).is_ok());
        assert!(
            miss(&h, one, &[s]).is_err(),
            "TLC compares the integer with the set"
        );
        let t = h.tuple(&[one]);
        assert!(
            miss(&h, t, &[s]).is_ok(),
            "a function is unequal to a set element"
        );
        assert!(miss(&h, a, &[]).is_ok());
    }

    #[test]
    fn tlc_comparisons_refuse_shapes_tlc_could_fail_on() {
        let store = Arc::new(Store::with_semantics(true, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let one = Handle::int(1).unwrap();
        let a = h.string("a").unwrap();
        let t = h.tuple(&[one]);
        let t2 = h.tuple(&[one, one]);
        let r = h.fcn_sorted(&[a], &[one]);
        let ra = h.fcn_sorted(&[a], &[a]);
        assert!(!equals(&h, t, t2).unwrap(), "tuples of any length compare");
        assert!(
            equals(&h, t, r).is_err(),
            "a tuple with a record is refused"
        );
        assert!(equals(&h, r, ra).is_err(), "a field of another class");
        let s1 = h.set_from(&mut vec![one]);
        let sa = h.set_from(&mut vec![a]);
        assert!(equals(&h, s1, sa).is_err(), "TLC compares 1 with \"a\"");
        let empty = h.set_from(&mut vec![]);
        assert!(!equals(&h, s1, empty).unwrap());
        assert!(check_set(&h, &[t, t2]).is_ok());
        let mut v = vec![s1, sa];
        let ss = h.set_from(&mut v);
        let elems = h.view(ss).data.to_vec();
        assert!(
            check_set(&h, &elems).is_err(),
            "{{1}} and {{\"a\"}} in one set"
        );
        assert!(miss(&h, t2, &[t]).is_ok());
        assert!(miss(&h, r, &[t]).is_err());
        // The default profile answers by D-009.
        let mut d = heap();
        let one = Handle::int(1).unwrap();
        let a = d.string("a").unwrap();
        let t = d.tuple(&[one]);
        let r = d.fcn_sorted(&[a], &[one]);
        assert!(!equals(&d, t, r).unwrap(), "D-009: a tuple is not a record");
        let s1 = d.set_from(&mut vec![one]);
        let sa = d.set_from(&mut vec![a]);
        assert!(!equals(&d, s1, sa).unwrap());
    }

    #[test]
    fn compound_set_normalization_is_guarded_in_both_profiles() {
        for semantics in [crate::Semantics::default(), crate::Semantics::tlc()] {
            let store = Arc::new(Store::with_semantics(true, semantics));
            let mut h = Heap::new(store, false, false);
            let one = Handle::int(1).unwrap();
            let two = Handle::int(2).unwrap();
            let yes = Handle::bool(true);
            let field = h.string("a").unwrap();
            let t = h.tuple(&[one, one]);
            let other = h.tuple(&[yes, two]);
            let later = h.tuple(&[one, yes]);
            let r = h.fcn_sorted(&[field], &[one]);
            let rb = h.fcn_sorted(&[field], &[yes]);
            let s = h.set_from(&mut vec![one]);
            let sb = h.set_from(&mut vec![yes]);
            for pair in [[t, other], [t, later], [r, rb], [s, sb]] {
                let set = h.set_from(&mut pair.to_vec());
                let err = check_set(&h, h.view(set).data).unwrap_err();
                assert_eq!(err.kind(), crate::ErrorKind::Unsupported);
            }
            let compatible = h.tuple(&[two, two]);
            let set = h.set_from(&mut vec![t, compatible]);
            assert!(check_set(&h, h.view(set).data).is_ok());
        }
    }

    #[test]
    fn mixed_sets_are_refused() {
        let mut h = heap();
        let one = Handle::int(1).unwrap();
        let a = h.string("a").unwrap();
        let mut v = vec![one, a];
        let s = h.set_from(&mut v);
        let elems = h.view(s).data.to_vec();
        assert!(check_set(&h, &elems).is_err());
        let t = h.tuple(&[one]);
        let r = h.fcn_sorted(&[a], &[one]);
        let mut v = vec![t, r];
        let s = h.set_from(&mut v);
        let elems = h.view(s).data.to_vec();
        assert!(check_set(&h, &elems).is_err());
        let t2 = h.tuple(&[one, one]);
        let mut v = vec![t, t2];
        let s = h.set_from(&mut v);
        let elems = h.view(s).data.to_vec();
        assert!(check_set(&h, &elems).is_ok());
    }
}
