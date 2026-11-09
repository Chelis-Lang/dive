//! Equality, membership and set-construction rules of the default profile
//! (`02-semantic-contract.md` C1–C3, D-009), as `dive-eval` applies
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
//! - Core finite-set construction is canonical (ADR 0153, D-028). Nominal
//!   compat retains existing checked-comparison refusals instead of reproducing
//!   TLC's mutable construction history.
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

/// The materialized-tree invariant required by mathematical identity consumers.
pub(crate) fn materialized(heap: &Heap, value: Handle, depth: u8) -> Result<bool, Error> {
    heap.work_poll()?;
    if depth == 64 || value.is_empty() {
        return Ok(false);
    }
    if !value.is_compound() {
        return Ok(value
            .as_str_id()
            .or_else(|| value.as_model_id())
            .is_none_or(|id| heap.known_name(id)));
    }
    let view = heap.view(value);
    if view.flags & crate::node::flags::CONCRETE != 0 {
        return Ok(true);
    }
    if view.is_lazy()
        || !matches!(
            view.kind,
            Kind::Set | Kind::IntFcn | Kind::Fcn | Kind::BigInt
        )
    {
        return Ok(false);
    }
    if view.kind == Kind::BigInt {
        return Ok(true);
    }
    for &child in view.data {
        if !materialized(heap, child, depth + 1)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Requires mathematical identities rather than structural symbolic fallback.
pub(crate) fn require_materialized(heap: &Heap, values: &[Handle]) -> Result<(), Error> {
    for &value in values {
        if !materialized(heap, value, 0)? {
            return Err(Error::unsupported(
                "mathematical identity of a symbolic or unclosed value (including nested set/function children)",
            ));
        }
    }
    Ok(())
}

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
    pub(crate) const fn is_scalar(self) -> bool {
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
    heap.model_type(h.as_model_id()?)
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
    require_materialized(heap, &[a, b])?;
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
    if let Some(witness) = memo.member_witness(heap.store(), set) {
        // A failed proof always takes the original full path. In particular,
        // no cached shape certificate changes the refusal or its demand site.
        if tuple_proof(heap, x, witness) || guard_member(heap, x, &[witness]).is_ok() {
            return miss_scalars(heap, x, elems);
        }
        return miss(heap, x, elems);
    }
    // Admission happens only after the demanded original miss succeeds.
    miss(heap, x, elems)?;
    if let Some(witness) = member_witness(heap, elems) {
        memo.remember_member(heap.store(), set, witness);
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

/// Adds a value's comparison constraints to one accumulated shape. This is
/// `unify(acc, shape(value))` without constructing a second tree or removing
/// and reinserting keys already present in the accumulator.
fn include_shape(heap: &Heap, acc: &mut Shape, h: Handle) -> Option<()> {
    use Shape as S;
    // The first value supplies the tree. An empty function can acquire the
    // shape of a non-record function, exactly as in `unify`.
    if matches!(acc, S::Hole | S::EmptyFcn) {
        let incoming = shape(heap, h)?;
        let old = std::mem::replace(acc, S::Hole);
        *acc = unify(old, incoming)?;
        return Some(());
    }
    match h.tag() {
        Tag::Bool | Tag::Int | Tag::Str | Tag::Model => {
            let incoming = shape(heap, h)?;
            return (incoming == S::Hole || *acc == incoming).then_some(());
        }
        Tag::Persistent | Tag::Transient | Tag::Empty => {}
    }
    if !h.is_compound() {
        return matches!(acc, S::Set(_)).then_some(());
    }
    let v = heap.view(h);
    match v.kind {
        Kind::BigInt => matches!(acc, S::Int).then_some(()),
        Kind::Set => {
            let S::Set(inner) = acc else {
                return None;
            };
            for &e in v.data {
                include_shape(heap, inner, e)?;
            }
            Some(())
        }
        Kind::Interval => {
            let S::Set(inner) = acc else {
                return None;
            };
            if **inner == S::Hole {
                **inner = S::Int;
            }
            matches!(**inner, S::Int).then_some(())
        }
        Kind::IntFcn if v.data.is_empty() => {
            matches!(acc, S::IntKeyed(_) | S::Gen(..)).then_some(())
        }
        Kind::IntFcn => {
            let S::IntKeyed(fields) = acc else {
                return None;
            };
            for (i, &e) in (v.lo..).zip(v.data) {
                include_shape(heap, fields.entry(i).or_insert(S::Hole), e)?;
            }
            Some(())
        }
        Kind::Fcn => {
            let (keys, vals) = (v.keys(), v.values());
            if keys.is_empty() {
                return matches!(acc, S::IntKeyed(_) | S::Gen(..)).then_some(());
            }
            if keys.iter().all(|k| k.as_str_id().is_some()) {
                let S::Rec(fields) = acc else {
                    return None;
                };
                for (k, &e) in keys.iter().zip(vals) {
                    include_shape(heap, fields.entry(k.as_str_id()?).or_insert(S::Hole), e)?;
                }
                return Some(());
            }
            if keys.iter().all(|k| k.as_int().is_some()) {
                let S::IntKeyed(fields) = acc else {
                    return None;
                };
                for (k, &e) in keys.iter().zip(vals) {
                    include_shape(heap, fields.entry(k.as_int()?).or_insert(S::Hole), e)?;
                }
                return Some(());
            }
            let S::Gen(ks, vs) = acc else {
                return None;
            };
            for (&k, &e) in keys.iter().zip(vals) {
                include_shape(heap, ks, k)?;
                include_shape(heap, vs, e)?;
            }
            Some(())
        }
        _ => None,
    }
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

/// A success-only proof against a witness whose complete shape is known to
/// exist from its certificate. It constructs no temporary Shape trees.
///
/// Identity is safe only under that witness-validity precondition. Model
/// holes remove constraints, but a hole in the witness does not establish
/// that the query has a shape: that case validates the supported query tree.
/// Tuple domains must match, except that an empty query adds no constraints
/// to a known-valid nonrecord tuple witness. Any unknown or mismatch returns
/// false to request the original witness/full guards, never a refusal or a
/// membership result.
fn tuple_proof(heap: &Heap, query: Handle, witness: Handle) -> bool {
    if query == witness {
        return true;
    }
    match (query.tag(), witness.tag()) {
        (Tag::Model, _) => {
            let Some(ty) = mv_type(heap, query) else {
                return true;
            };
            return witness.tag() == Tag::Model
                && mv_type(heap, witness).is_none_or(|other| ty == other);
        }
        (_, Tag::Model) => {
            return mv_type(heap, witness).is_none() && tuple_tree_valid(heap, query);
        }
        (Tag::Bool, Tag::Bool) | (Tag::Int, Tag::Int) | (Tag::Str, Tag::Str) => return true,
        _ => {}
    }
    if !query.is_compound() || !witness.is_compound() {
        return false;
    }
    let (a, b) = (heap.view(query), heap.view(witness));
    if a.kind != Kind::IntFcn || b.kind != Kind::IntFcn || a.lo != 1 || b.lo != 1 {
        return false;
    }
    a.data.is_empty()
        || (a.data.len() == b.data.len()
            && a.data
                .iter()
                .zip(b.data)
                .all(|(&x, &y)| tuple_proof(heap, x, y)))
}

/// Whether the query has a shape in the deliberately narrow tuple-tree
/// subset. Used when an untyped witness model value supplies no constraints.
fn tuple_tree_valid(heap: &Heap, query: Handle) -> bool {
    match query.tag() {
        Tag::Bool | Tag::Int | Tag::Str | Tag::Model => true,
        Tag::Persistent | Tag::Transient => {
            let value = heap.view(query);
            value.kind == Kind::IntFcn
                && value.lo == 1
                && value.data.iter().all(|&x| tuple_tree_valid(heap, x))
        }
        Tag::Empty => false,
    }
}

/// A success-only alternative to building `Shape::IntKeyed` maps. Every
/// function has the same integer domain and the same scalar class at each
/// key, so all of their complete shapes are equal. This proves the original
/// join succeeds; it supplies neither a comparison result nor TLC history.
///
/// The shared entry budget bounds unsuccessful probes too. Unknown keys,
/// nested values, model values, raw views and unsupported kinds return false
/// to leave the original guard and its exact refusal unchanged. In particular,
/// even identical handles are inspected: identity is not a shape certificate.
fn scalar_function_shapes(heap: &Heap, first: Handle, rest: &[Handle]) -> bool {
    const WORK: usize = 64;

    fn scalar(heap: &Heap, h: Handle) -> bool {
        match h.tag() {
            Tag::Bool | Tag::Int => true,
            Tag::Str => h
                .as_str_id()
                .is_some_and(|id| heap.store().strings().text(id).is_some()),
            _ => false,
        }
    }

    fn function(heap: &Heap, h: Handle) -> Option<crate::node::View<'_>> {
        if !h.is_compound() {
            return None;
        }
        let v = heap.view(h);
        if !v.kind.is_fcn()
            || v.flags & !(crate::node::flags::TUPLE | crate::node::flags::CONCRETE) != 0
        {
            return None;
        }
        Some(v)
    }

    let Some(base) = function(heap, first) else {
        return false;
    };
    let mut remaining = WORK;
    if base.len() >= remaining {
        return false;
    }
    remaining -= base.len() + 1;
    match base.kind {
        Kind::IntFcn => {
            if !base.data.is_empty()
                && i64::try_from(base.len() - 1)
                    .ok()
                    .and_then(|n| base.lo.checked_add(n))
                    .is_none()
            {
                return false;
            }
        }
        Kind::Fcn => {
            let mut previous = None;
            for &key in base.keys() {
                let Some(key) = key.as_int() else {
                    return false;
                };
                if previous.is_some_and(|p| p >= key) {
                    return false;
                }
                previous = Some(key);
            }
        }
        _ => return false,
    }
    if !base.values().iter().all(|&h| scalar(heap, h)) {
        return false;
    }
    for &h in rest {
        let Some(other) = function(heap, h) else {
            return false;
        };
        if other.kind != base.kind
            || other.len() != base.len()
            || other.lo != base.lo
            || other.len() >= remaining
        {
            return false;
        }
        remaining -= other.len() + 1;
        if other.keys() != base.keys()
            || !base
                .values()
                .iter()
                .zip(other.values())
                .all(|(&a, &b)| a.tag() == b.tag() && scalar(heap, b))
        {
            return false;
        }
    }
    true
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
    if scalar_function_shapes(heap, a, &[b]) {
        return Ok(());
    }
    let ok = shape(heap, a).is_some_and(|mut acc| include_shape(heap, &mut acc, b).is_some());
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
    if scalar_function_shapes(heap, x, elems) {
        return Ok(());
    }
    let ok = (|| {
        let mut acc = shape(heap, x)?;
        for &e in elems {
            include_shape(heap, &mut acc, e)?;
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
    if scalar_function_shapes(heap, elems[0], &elems[1..]) {
        return Ok(());
    }
    let tlc = heap.store().semantics().tlc_comparisons;
    let ok = (|| {
        let mut acc = Shape::Hole;
        for &e in elems {
            include_shape(heap, &mut acc, e)?;
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

    fn original_guard_member(heap: &Heap, x: Handle, elems: &[Handle]) -> Result<(), Error> {
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

    fn original_miss(heap: &Heap, x: Handle, elems: &[Handle]) -> Result<(), Error> {
        original_guard_member(heap, x, elems)?;
        miss_scalars(heap, x, elems)
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

    fn original_guard_set(heap: &Heap, elems: &[Handle]) -> Result<(), Error> {
        if elems.len() < 2 {
            return Ok(());
        }
        let ok = elems
            .iter()
            .try_fold(Shape::Hole, |acc, &h| unify(acc, shape(heap, h)?))
            .is_some();
        if ok {
            Ok(())
        } else if heap.store().semantics().tlc_comparisons {
            Err(guard_refusal("a set of values of different shapes"))
        } else {
            Err(Error::unsupported(
                "a set of values of different shapes: TLC's checked normalization may fail (C1, C2; the exact comparison order is not built)",
            ))
        }
    }

    #[test]
    fn scalar_function_proofs_preserve_original_guards_in_both_profiles() {
        for semantics in [crate::Semantics::default(), crate::Semantics::tlc()] {
            let store = Arc::new(Store::with_semantics(true, semantics));
            let mut h = Heap::new(store, false, false);
            let mut values = shape_values(&mut h);
            let one = Handle::int(1).unwrap();
            let three = Handle::int(3).unwrap();
            let text = h.string("a").unwrap();
            values.extend([
                h.tuple_literal(&[one, Handle::TRUE, text]),
                h.tuple(&[three, Handle::FALSE, text]),
                h.fcn_sorted(&[one, three], &[Handle::TRUE, one]),
                h.fcn_sorted(&[one, three], &[Handle::FALSE, three]),
                h.fcn_sorted(&[one, Handle::int(4).unwrap()], &[Handle::TRUE, one]),
                h.set_enum(&[one, Handle::TRUE, one]),
            ]);
            let persistent: Vec<_> = values.iter().map(|&v| h.intern_exact(v).unwrap()).collect();
            values.extend(persistent);
            let mut proved = 0;
            for &a in &values {
                for &b in &values {
                    let pair = [a, b];
                    if scalar_function_shapes(&h, a, &[b]) {
                        proved += 1;
                        assert_eq!(shape(&h, a), shape(&h, b));
                        assert!(original_guard_set(&h, &pair).is_ok());
                    }
                    assert_eq!(guard_set(&h, &pair), original_guard_set(&h, &pair));
                    assert_eq!(
                        guard_member(&h, a, &[b]),
                        original_guard_member(&h, a, &[b])
                    );
                    let expected = if !semantics.tlc_comparisons
                        || shape(&h, a)
                            .is_some_and(|s| shape(&h, b).and_then(|t| unify(s, t)).is_some())
                    {
                        Ok(())
                    } else {
                        Err(guard_refusal(
                            "an equality between values of different shapes",
                        ))
                    };
                    assert_eq!(guard_equals(&h, a, b), expected);
                }
            }
            assert!(proved > 20, "exercise successful proofs: {proved}");
            assert!(!h.poisoned());
        }
    }

    #[test]
    fn scalar_function_proof_bounds_work_and_checks_every_field_and_member() {
        let store = Arc::new(Store::with_semantics(true, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let one = Handle::int(1).unwrap();
        let text = h.string("owned").unwrap();
        let a = h.tuple(&[one, Handle::TRUE, text]);
        let b = h.tuple(&[Handle::int(2).unwrap(), Handle::FALSE, text]);
        assert!(scalar_function_shapes(&h, a, &[b, a]));
        let wrong = h.tuple(&[one, Handle::TRUE, one]);
        assert!(!scalar_function_shapes(&h, a, &[b, wrong]));
        assert_eq!(
            guard_set(&h, &[a, b, wrong]),
            original_guard_set(&h, &[a, b, wrong])
        );
        let raw = h.set_enum(&[one, Handle::TRUE]);
        let nested = h.tuple(&[raw]);
        assert!(!scalar_function_shapes(&h, nested, &[nested]));
        assert!(guard_set(&h, &[nested, nested]).is_err());
        let unknown = h.tuple(&[Handle::string(u32::MAX)]);
        assert!(!scalar_function_shapes(&h, unknown, &[unknown]));
        let large = h.tuple(&[one; 64]);
        assert!(!scalar_function_shapes(&h, large, &[large]));
        assert_eq!(
            guard_set(&h, &[large, large]),
            original_guard_set(&h, &[large, large])
        );
        assert!(!scalar_function_shapes(&h, a, &[b; 16]));
        assert_eq!(
            guard_member(&h, a, &[b; 16]),
            original_guard_member(&h, a, &[b; 16])
        );
        // A shared invalid canonical subtree also cannot acquire a proof.
        let invalid = h.set_from(&mut vec![one, Handle::TRUE]);
        let invalid = h.intern_exact(invalid).unwrap();
        let nested = h.tuple(&[invalid]);
        assert!(!scalar_function_shapes(&h, nested, &[nested]));
        assert!(guard_set(&h, &[nested, nested]).is_err());
        assert!(!h.poisoned());
    }

    #[test]
    fn scalar_function_proof_rejects_foreign_and_stale_handles() {
        let one = Handle::int(1).unwrap();
        let mut source = heap();
        let value = source.tuple(&[one]);
        let foreign = source.intern_exact(value).unwrap();
        let other = heap();
        assert!(!scalar_function_shapes(&other, foreign, &[foreign]));
        assert!(other.poisoned(), "the original ownership check is consumed");
        source.reset();
        assert!(!scalar_function_shapes(&source, value, &[value]));
        assert!(source.poisoned(), "the original stale check is consumed");
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
    fn tuple_proof_successes_match_the_original_guard_for_valid_witnesses() {
        let store = Arc::new(Store::with_semantics(true, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let mut values = shape_values(&mut h);
        let persistent: Vec<_> = values.iter().map(|&v| h.intern_exact(v).unwrap()).collect();
        values.extend(persistent);
        let mut proved = 0;
        let mut checked = 0;
        for &witness in &values {
            if shape(&h, witness).is_none() {
                continue;
            }
            for &query in &values {
                checked += 1;
                if tuple_proof(&h, query, witness) {
                    proved += 1;
                    assert!(
                        original_guard_member(&h, query, &[witness]).is_ok(),
                        "invalid success for {query:?}, {witness:?}"
                    );
                    assert!(
                        unify(shape(&h, query).unwrap(), shape(&h, witness).unwrap()).is_some()
                    );
                }
            }
        }
        assert!(proved > 0);
        assert!(proved < checked, "unknown cases must retain the old guard");
        eprintln!("tuple proof coverage: {proved} successes / {checked} valid-witness pairs");
    }

    #[test]
    fn tuple_proof_covers_sailfish_vertex_and_edge_shapes() {
        let store = Arc::new(Store::with_semantics(true, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let genesis = h.tuple(&[]);
        let mut vertices = vec![genesis];
        for name in ["n1", "n2", "n3"] {
            let node = Handle::model(h.store().strings().intern(name).unwrap());
            for round in 1..=5 {
                vertices.push(h.tuple(&[node, Handle::int(round).unwrap()]));
            }
        }
        let witness = vertices[1];
        let edge_witness = h.tuple(&[witness, witness]);
        let mut proved = 0;
        for &query in &vertices {
            assert!(tuple_proof(&h, query, witness));
            assert!(original_guard_member(&h, query, &[witness]).is_ok());
            proved += 1;
        }
        for &source in &vertices[1..] {
            for &target in &vertices {
                let query = h.tuple(&[source, target]);
                assert!(tuple_proof(&h, query, edge_witness));
                assert!(original_guard_member(&h, query, &[edge_witness]).is_ok());
                proved += 1;
            }
        }
        assert_eq!(proved, 256);
        eprintln!("Sailfish-shaped tuple proof coverage: {proved} successes / 256 pairs");
    }

    #[test]
    fn tuple_proof_leaves_domains_and_holes_do_not_hide_refusals() {
        let store = Arc::new(Store::with_semantics(true, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        let text = h.string("a").unwrap();
        let untyped = Handle::model(h.store().strings().intern("u").unwrap());
        let typed_a = Handle::model(h.store().strings().intern("A_x").unwrap());
        let typed_b = Handle::model(h.store().strings().intern("B_x").unwrap());
        assert!(tuple_proof(&h, typed_a, untyped));
        assert!(tuple_proof(&h, untyped, typed_a));
        assert!(!tuple_proof(&h, typed_a, typed_b));
        assert!(!tuple_proof(&h, typed_a, one));
        assert!(!tuple_proof(&h, one, typed_a));
        assert!(!tuple_proof(&h, Handle::TRUE, one));
        assert!(!tuple_proof(&h, one, text));
        let invalid = h.set_from(&mut vec![one, text]);
        let empty = h.tuple(&[]);
        let witness = h.tuple(&[one]);
        let query = h.tuple(&[two]);
        assert!(tuple_proof(&h, query, witness));
        assert!(tuple_proof(&h, empty, witness));
        assert!(!tuple_proof(&h, query, empty));
        let different_domain = h.int_fcn(0, &[one]);
        assert!(!tuple_proof(&h, different_domain, witness));
        let invalid_extra_position = h.tuple(&[one, invalid]);
        assert!(!tuple_proof(&h, invalid_extra_position, witness));
        assert!(original_guard_member(&h, invalid_extra_position, &[witness]).is_err());
        let record = h.record(&[text], &[one]);
        assert!(!tuple_proof(&h, empty, record));
        let hole_witness = h.tuple(&[untyped]);
        let invalid_query = h.tuple(&[invalid]);
        assert!(!tuple_proof(&h, invalid_query, hole_witness));
        assert!(original_guard_member(&h, invalid_query, &[hole_witness]).is_err());
        let nested_query = h.tuple(&[query]);
        assert!(tuple_proof(&h, nested_query, hole_witness));
    }

    #[test]
    fn uncertified_shared_invalid_subtrees_cannot_use_tuple_identity_proofs() {
        let store = Arc::new(Store::with_semantics(true, crate::Semantics::tlc()));
        let mut h = Heap::new(store, false, false);
        let one = Handle::int(1).unwrap();
        let two = Handle::int(2).unwrap();
        let text = h.string("a").unwrap();
        let invalid = h.set_from(&mut vec![one, text]);
        let invalid = h.intern_exact(invalid).unwrap();
        let witness = h.tuple(&[invalid, one]);
        let set = h.set_from(&mut vec![witness]);
        let set = h.intern_exact(set).unwrap();
        let query = h.tuple(&[invalid, two]);
        let elems = h.view(set).data;
        let expected = original_miss(&h, query, elems);
        assert!(expected.is_err());
        let mut memo = crate::memo::Memo::new(&[]);
        memo.reserve_members(true);
        for _ in 0..2 {
            assert_eq!(miss_cached(&h, &mut memo, query, set, elems), expected);
            assert_eq!(memo.member_witness(h.store(), set), None);
        }
        assert!(!h.poisoned());
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
        assert!(original_guard_member(&h, query, &[right]).is_err());
        assert!(original_guard_member(&h, query, &[left]).is_ok());
        assert!(original_guard_member(&h, query, &[left, right]).is_err());
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
                        let expected = original_miss(&h, x, elems);
                        assert_eq!(miss_cached(&h, &mut memo, x, set, elems), expected);
                        assert_eq!(miss_cached(&h, &mut memo, x, set, elems), expected);
                        admitted += usize::from(memo.member_witness(h.store(), set).is_some());
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
            let expected = original_miss(&h, query, elems);
            assert!(expected.is_err());
            assert_eq!(miss_cached(&h, &mut memo, query, set, elems), expected);
            assert_eq!(memo.bytes(), 0, "a refusal does not admit a certificate");
            let compatible = h.tuple(&[two]);
            let elems = h.view(set).data;
            assert_eq!(miss_cached(&h, &mut memo, compatible, set, elems), Ok(()));
            assert!(memo.member_witness(h.store(), set).is_some());
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
    fn streaming_shape_constraints_match_the_original_guard() {
        let mut h = heap();
        let (one, two, three) = (
            Handle::int(1).unwrap(),
            Handle::int(2).unwrap(),
            Handle::int(3).unwrap(),
        );
        let (a, b) = (h.string("a").unwrap(), h.string("b").unwrap());
        let mv = |name| Handle::model(h.store().strings().intern(name).unwrap());
        let mut values = vec![
            Handle::TRUE,
            Handle::FALSE,
            one,
            two,
            a,
            b,
            mv("u"),
            mv("v"),
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
            h.fcn_sorted(&[Handle::FALSE, Handle::TRUE], &[one, two]),
            h.fcn_sorted(&[one, a], &[one, two]),
            h.set_from(&mut vec![]),
            h.set_from(&mut vec![one, a]),
            h.interval(1, 3),
            h.set_enum(&[one, two]),
            h.symbolic(Kind::Nat, 0, &[]),
            h.symbolic(Kind::SeqSet, 0, &[one]),
        ];
        // Include nested disagreements, empty holes, different tuple lengths,
        // record fields and generic-function domains, and exact persistent
        // copies alongside transient values and literal views.
        for _ in 0..2 {
            let previous = values.clone();
            for &v in &previous {
                values.push(h.tuple(&[v]));
                values.push(h.record(&[a], &[v]));
                values.push(h.set_from(&mut vec![v]));
            }
        }
        let exact: Vec<_> = values.iter().map(|&v| h.intern_exact(v).unwrap()).collect();
        values.extend(exact);
        let original = |items: &[Handle]| {
            items
                .iter()
                .try_fold(Shape::Hole, |acc, &v| unify(acc, shape(&h, v)?))
        };
        let streaming = |items: &[Handle]| {
            let mut acc = Shape::Hole;
            for &v in items {
                include_shape(&h, &mut acc, v)?;
            }
            Some(acc)
        };
        for &a in &values {
            for &b in &values {
                assert_eq!(streaming(&[a, b]), original(&[a, b]), "{a:?}, {b:?}");
            }
        }
        // Longer input orders exercise partial unification and late refusals.
        let mut seed = 0x588_593_u64;
        for _ in 0..4096 {
            let mut items = Vec::new();
            for _ in 0..5 {
                seed = seed.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1);
                let index = usize::try_from(seed >> 32).unwrap() % values.len();
                items.push(values[index]);
            }
            assert_eq!(streaming(&items), original(&items), "{items:?}");
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
