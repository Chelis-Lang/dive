//! TLC's checked comparison, equality and membership.
//!
//! These are ports of `compareTo`, `equals` and `member` of the classes in
//! `tlc2.value.impl` (and `tlc2.module.Naturals`, `Integers`, `Strings`,
//! `Sequences`), tlaplus `94d0c50`. They raise the errors TLC raises, with TLC's
//! messages. TLC dispatches on the class of the left operand (`this`); so do these
//! functions, mapping dive's representation to TLC's class:
//!
//! | dive | TLC |
//! |---|---|
//! | enumerated set | `SetEnumValue` |
//! | `a..b` | `IntervalValue` |
//! | `SUBSET S` | `SubsetValue` |
//! | `[S -> T]` | `SetOfFcnsValue` |
//! | `[a : S]` | `SetOfRcdsValue` |
//! | `S \X T` | `SetOfTuplesValue` |
//! | `Nat`, `Int`, `STRING`, `Seq(S)` | `UserValue` |
//! | tuple | `TupleValue` |
//! | record | `RecordValue` |
//! | general function | `FcnRcdValue` with an explicit domain |

#![allow(
    clippy::many_single_char_names,
    reason = "ports keep short operand names (a, b, s, t, x, y)"
)]

use std::cmp::Ordering;

use crate::atom::Atom;
use crate::ctx::Ctx;
use crate::error::{ErrorKind, Result, ValueError};
use crate::print::{ppr, to_tlc_string_lenient};
use crate::value::{Fcn, FcnRepr, Set, SetRepr, Value};

/// `Values.ppr(v.toString())`.
fn p(v: &Value, ctx: Ctx) -> String {
    ppr(&to_tlc_string_lenient(v, ctx))
}

fn fail(msg: String) -> ValueError {
    ValueError::new(ErrorKind::Incomparable, msg)
}

fn fail_member(msg: String) -> ValueError {
    ValueError::new(ErrorKind::Membership, msg)
}

/// TLC's `UserValue` objects.
#[derive(Clone, Copy, PartialEq, Eq)]
enum User {
    Nat,
    Int,
    Strings,
    Seq,
}

fn user_of(s: &Set) -> Option<User> {
    match s.repr() {
        SetRepr::Nat => Some(User::Nat),
        SetRepr::Int => Some(User::Int),
        SetRepr::Strings => Some(User::Strings),
        SetRepr::Seq(_) => Some(User::Seq),
        _ => None,
    }
}

fn user_name(u: User, v: &Value, ctx: Ctx) -> String {
    match u {
        User::Nat => "Nat".to_owned(),
        User::Int => "Int".to_owned(),
        User::Strings => "STRING".to_owned(),
        User::Seq => p(v, ctx),
    }
}

/// `ModelValue.modelValueCompareTo`: `mv` is the right operand of a non-model `this`.
fn mv_compare_rev(mv: &Atom, mv_val: &Value, this: &Value, ctx: Ctx) -> Result<Ordering> {
    if mv.mv_type().is_some() {
        return Err(fail(format!(
            "Attempted to compare the typed model value {} and the non-model value\n{}",
            p(mv_val, ctx),
            p(this, ctx)
        )));
    }
    Ok(Ordering::Greater)
}

/// `ModelValue.modelValueEquals`.
fn mv_equals_rev(mv: &Atom, mv_val: &Value, this: &Value, ctx: Ctx) -> Result<bool> {
    if mv.mv_type().is_some() {
        return Err(fail(format!(
            "Attempted to check equality of the typed model value {} and the non-model value\n{}",
            p(mv_val, ctx),
            p(this, ctx)
        )));
    }
    Ok(false)
}

/// `ModelValue.modelValueMember`.
fn mv_member(mv: &Atom, mv_val: &Value, set: &Value, ctx: Ctx) -> Result<bool> {
    if mv.mv_type().is_some() {
        return Err(fail_member(format!(
            "Attempted to check if the typed model value {} is an element of\n{}",
            p(mv_val, ctx),
            p(set, ctx)
        )));
    }
    Ok(false)
}

/// `Value.toSetEnum()`: the normalized elements of a finite set, or `None` for
/// non-sets and `UserValue`s.
fn to_set_enum(v: &Value, ctx: Ctx) -> Result<Option<std::sync::Arc<[Value]>>> {
    match v {
        Value::Set(s) if user_of(s).is_none() => s.elements(ctx).map(Some),
        _ => Ok(None),
    }
}

fn enum_value(elems: &std::sync::Arc<[Value]>, ctx: Ctx) -> Value {
    Value::Set(Set::from_sorted(ctx.order, elems.to_vec()))
}

/// A view of a function as TLC's `FcnRcdValue`: interval domain `1..n` or explicit.
enum FcnView<'a> {
    Intv(&'a [Value]),
    Array(Vec<(Value, &'a Value)>),
}

impl FcnView<'_> {
    fn len(&self) -> usize {
        match self {
            FcnView::Intv(v) => v.len(),
            FcnView::Array(p) => p.len(),
        }
    }
    fn value(&self, i: usize) -> &Value {
        match self {
            FcnView::Intv(v) => &v[i],
            FcnView::Array(p) => p[i].1,
        }
    }
}

/// `Value.toFcnRcd()`.
fn to_fcn(v: &Value) -> Option<FcnView<'_>> {
    match v {
        Value::Fcn(f) => Some(match &f.0.repr {
            FcnRepr::Tuple(e) => FcnView::Intv(e),
            _ => FcnView::Array(f.iter().collect()),
        }),
        _ => None,
    }
}

/// `Value.toTuple()`: tuples, and the (impossible in dive) records with no field.
fn to_tuple(v: &Value) -> Option<&[Value]> {
    match v {
        Value::Fcn(f) => f.as_tuple(),
        _ => None,
    }
}

/// `Value.toRcd()`: records, and the empty tuple as the empty record.
fn to_rcd(v: &Value) -> Option<&[(Atom, Value)]> {
    match v {
        Value::Fcn(f) => match &f.0.repr {
            FcnRepr::Record(fs) => Some(fs),
            FcnRepr::Tuple(e) if e.is_empty() => Some(&[]),
            _ => None,
        },
        _ => None,
    }
}

fn idx_value(i: usize) -> i64 {
    i64::try_from(i).unwrap_or(i64::MAX) + 1
}

/// TLC's `a.compareTo(b)`.
///
/// # Errors
/// The error TLC raises when it refuses to compare the two values.
pub fn compare(a: &Value, b: &Value, ctx: Ctx) -> Result<Ordering> {
    match a {
        Value::Bool(x) => match b {
            Value::Bool(y) => Ok(x.cmp(y)),
            Value::Model(m) => mv_compare_rev(m, b, a, ctx),
            _ => Err(fail(format!(
                "Attempted to compare boolean {} with non-boolean:\n{}",
                p(a, ctx),
                p(b, ctx)
            ))),
        },
        Value::Int(_) | Value::BigInt(_) => match (a.as_int(), b.as_int()) {
            (Some(x), Some(y)) => Ok(x.cmp(&y)),
            _ => match b {
                Value::Model(m) => mv_compare_rev(m, b, a, ctx),
                _ => Err(fail(format!(
                    "Attempted to compare integer {} with non-integer:\n{}",
                    p(a, ctx),
                    p(b, ctx)
                ))),
            },
        },
        Value::Str(x) => match b {
            Value::Str(y) => Ok(x.cmp_with(y, ctx.order)),
            Value::Model(m) => mv_compare_rev(m, b, a, ctx),
            _ => Err(fail(format!(
                "Attempted to compare string {} with non-string:\n{}",
                p(a, ctx),
                p(b, ctx)
            ))),
        },
        Value::Model(x) => compare_mv(x, a, b, ctx),
        Value::Set(s) => compare_set(s, a, b, ctx),
        Value::Fcn(f) => compare_fcn(f, a, b, ctx),
    }
}

fn compare_mv(x: &Atom, a: &Value, b: &Value, ctx: Ctx) -> Result<Ordering> {
    match (x.mv_type(), b) {
        (None, Value::Model(y)) => Ok(x.cmp_with(y, ctx.order)),
        (None, _) => Ok(Ordering::Less),
        (Some(t), Value::Model(y)) => {
            if y.mv_type().is_none_or(|u| u == t) {
                Ok(x.cmp_with(y, ctx.order))
            } else {
                Err(fail(format!(
                    "Attempted to compare the differently-typed model values {} and {}",
                    p(a, ctx),
                    p(b, ctx)
                )))
            }
        }
        (Some(_), _) => Err(fail(format!(
            "Attempted to compare the typed model value {} and non-model value\n{}",
            p(a, ctx),
            p(b, ctx)
        ))),
    }
}

fn user_compare(u: User, a: &Value, b: &Value, ctx: Ctx) -> Result<Ordering> {
    let other = match b {
        Value::Set(t) => user_of(t).map(|v| (v, t)),
        _ => None,
    };
    let Some((v, t)) = other else {
        if matches!(b, Value::Model(_)) {
            return Ok(Ordering::Greater);
        }
        return Err(fail(format!(
            "Attempted to compare overridden value {} with non-overridden value:\n{}",
            p(a, ctx),
            p(b, ctx)
        )));
    };
    let module_err = || {
        ValueError::new(
            ErrorKind::ModuleCompare,
            format!(
                "Attempted to compare {} with the value\n{}",
                user_name(u, a, ctx),
                p(b, ctx)
            ),
        )
    };
    match (u, v) {
        (User::Nat, User::Nat) | (User::Int, User::Int) | (User::Strings, User::Strings) => {
            Ok(Ordering::Equal)
        }
        (User::Nat, User::Int) => Ok(Ordering::Less),
        (User::Int, User::Nat) => Ok(Ordering::Greater),
        (User::Seq, User::Seq) => match (a, t.repr()) {
            (Value::Set(s), SetRepr::Seq(r2)) => match s.repr() {
                SetRepr::Seq(r1) => compare(r1, r2, ctx),
                _ => Err(module_err()),
            },
            _ => Err(module_err()),
        },
        _ => Err(module_err()),
    }
}

fn compare_set(s: &Set, a: &Value, b: &Value, ctx: Ctx) -> Result<Ordering> {
    if let Some(u) = user_of(s) {
        return user_compare(u, a, b, ctx);
    }
    match (s.repr(), b) {
        (SetRepr::Interval(l1, h1), Value::Set(t)) => {
            if let SetRepr::Interval(l2, h2) = t.repr() {
                let (n1, n2) = (s.size(ctx)?, t.size(ctx)?);
                if n1 != n2 {
                    return Ok(n1.cmp(&n2));
                }
                if n1 == 0 {
                    return Ok(Ordering::Equal);
                }
                let _ = (h1, h2);
                return Ok(l1.cmp(l2));
            }
        }
        (SetRepr::Subset(x), Value::Set(t)) => {
            if let SetRepr::Subset(y) = t.repr() {
                return compare(x, y, ctx);
            }
        }
        _ => {}
    }
    let mine = s.elements(ctx)?;
    let Some(theirs) = to_set_enum(b, ctx)? else {
        if let Value::Model(m) = b {
            return mv_compare_rev(m, b, a, ctx);
        }
        return Err(fail(format!(
            "Attempted to compare the set {} with the value:\n{}",
            p(&enum_value(&mine, ctx), ctx),
            p(b, ctx)
        )));
    };
    if mine.len() != theirs.len() {
        return Ok(mine.len().cmp(&theirs.len()));
    }
    for (x, y) in mine.iter().zip(theirs.iter()) {
        let c = compare(x, y, ctx)?;
        if c != Ordering::Equal {
            return Ok(c);
        }
    }
    Ok(Ordering::Equal)
}

fn compare_fcn(f: &Fcn, a: &Value, b: &Value, ctx: Ctx) -> Result<Ordering> {
    match &f.0.repr {
        FcnRepr::Tuple(e) => {
            if let Some(t) = to_tuple(b) {
                if e.len() != t.len() {
                    return Ok(e.len().cmp(&t.len()));
                }
                for (x, y) in e.iter().zip(t) {
                    let c = compare(x, y, ctx)?;
                    if c != Ordering::Equal {
                        return Ok(c);
                    }
                }
                return Ok(Ordering::Equal);
            }
            compare_fcnrcd(&FcnView::Intv(e), a, b, ctx)
        }
        FcnRepr::Record(fs) => {
            let Some(r) = to_rcd(b) else {
                if let Value::Model(m) = b {
                    return mv_compare_rev(m, b, a, ctx);
                }
                return Err(fail(format!(
                    "Attempted to compare record:\n{}\nwith non-record\n{}",
                    p(a, ctx),
                    p(b, ctx)
                )));
            };
            if fs.len() != r.len() {
                return Ok(fs.len().cmp(&r.len()));
            }
            for ((n1, _), (n2, _)) in fs.iter().zip(r) {
                let c = n1.cmp_with(n2, ctx.order);
                if c != Ordering::Equal {
                    return Ok(c);
                }
            }
            for ((_, v1), (_, v2)) in fs.iter().zip(r) {
                let c = compare(v1, v2, ctx)?;
                if c != Ordering::Equal {
                    return Ok(c);
                }
            }
            Ok(Ordering::Equal)
        }
        FcnRepr::General(_) => {
            let view = FcnView::Array(f.iter().collect());
            compare_fcnrcd(&view, a, b, ctx)
        }
    }
}

fn non_int_domain(d: &Value, ctx: Ctx, an: bool) -> ValueError {
    let article = if an { "an integer" } else { "integer" };
    fail(format!(
        "Attempted to compare {article} with non-integer:\n{}.",
        p(d, ctx)
    ))
}

/// `FcnRcdValue.compareTo`, with `this` viewed as `me`.
fn compare_fcnrcd(me: &FcnView<'_>, a: &Value, b: &Value, ctx: Ctx) -> Result<Ordering> {
    let Some(other) = to_fcn(b) else {
        if let Value::Model(m) = b {
            return mv_compare_rev(m, b, a, ctx);
        }
        return Err(fail(format!(
            "Attempted to compare the function {} with the value:\n{}",
            p(a, ctx),
            p(b, ctx)
        )));
    };
    if me.len() != other.len() {
        return Ok(me.len().cmp(&other.len()));
    }
    let n = me.len();
    match (me, &other) {
        (FcnView::Intv(_), _) if n == 0 => return Ok(Ordering::Equal),
        (FcnView::Intv(_), FcnView::Intv(_)) => {}
        (FcnView::Intv(_), FcnView::Array(ps)) => {
            for (i, (d, _)) in ps.iter().enumerate() {
                let Value::Int(k) = d else {
                    return Err(non_int_domain(d, ctx, false));
                };
                let c = idx_value(i).cmp(k);
                if c != Ordering::Equal {
                    return Ok(c);
                }
            }
        }
        (FcnView::Array(ps), FcnView::Intv(_)) => {
            for (i, (d, _)) in ps.iter().enumerate() {
                let Value::Int(k) = d else {
                    return Err(non_int_domain(d, ctx, false));
                };
                let c = k.cmp(&idx_value(i));
                if c != Ordering::Equal {
                    return Ok(c);
                }
            }
        }
        (FcnView::Array(p1), FcnView::Array(p2)) => {
            for ((d1, _), (d2, _)) in p1.iter().zip(p2) {
                let c = compare(d1, d2, ctx)?;
                if c != Ordering::Equal {
                    return Ok(c);
                }
            }
        }
    }
    for i in 0..n {
        let c = compare(me.value(i), other.value(i), ctx)?;
        if c != Ordering::Equal {
            return Ok(c);
        }
    }
    Ok(Ordering::Equal)
}

/// TLC's `a.equals(b)`: the `=` of TLA+ as TLC evaluates it.
///
/// # Errors
/// The error TLC raises when it refuses to test the two values for equality, such as
/// `Attempted to check equality of integer 1 with non-integer:\n"a"`.
pub fn equals(a: &Value, b: &Value, ctx: Ctx) -> Result<bool> {
    match a {
        Value::Bool(x) => match b {
            Value::Bool(y) => Ok(x == y),
            Value::Model(m) => mv_equals_rev(m, b, a, ctx),
            _ => Err(fail(format!(
                "Attempted to compare equality of boolean {} with non-boolean:\n{}",
                p(a, ctx),
                p(b, ctx)
            ))),
        },
        Value::Int(_) | Value::BigInt(_) => match (a.as_int(), b.as_int()) {
            (Some(x), Some(y)) => Ok(x == y),
            _ => match b {
                Value::Model(m) => mv_equals_rev(m, b, a, ctx),
                _ => Err(fail(format!(
                    "Attempted to check equality of integer {} with non-integer:\n{}",
                    p(a, ctx),
                    p(b, ctx)
                ))),
            },
        },
        Value::Str(x) => match b {
            Value::Str(y) => Ok(x == y),
            Value::Model(m) => mv_equals_rev(m, b, a, ctx),
            _ => Err(fail(format!(
                "Attempted to check equality of string {} with non-string:\n{}",
                p(a, ctx),
                p(b, ctx)
            ))),
        },
        Value::Model(x) => match (x.mv_type(), b) {
            (None, Value::Model(y)) => Ok(x == y),
            (None, _) => Ok(false),
            (Some(t), Value::Model(y)) => {
                if y.mv_type().is_none_or(|u| u == t) {
                    Ok(x == y)
                } else {
                    Err(fail(format!(
                        "Attempted to check equality of the differently-typed model values {} and {}",
                        p(a, ctx),
                        p(b, ctx)
                    )))
                }
            }
            (Some(_), _) => Err(fail(format!(
                "Attempted to check equality of typed model value {} and non-model value\n{}",
                p(a, ctx),
                p(b, ctx)
            ))),
        },
        Value::Set(s) => equals_set(s, a, b, ctx),
        Value::Fcn(f) => equals_fcn(f, a, b, ctx),
    }
}

fn equals_set(s: &Set, a: &Value, b: &Value, ctx: Ctx) -> Result<bool> {
    if user_of(s).is_some() {
        return Ok(compare(a, b, ctx)? == Ordering::Equal);
    }
    if let Value::Set(t) = b {
        match (s.repr(), t.repr()) {
            (SetRepr::Interval(l1, h1), SetRepr::Interval(l2, h2)) => {
                if s.size(ctx)? == 0 {
                    return Ok(t.size(ctx)? == 0);
                }
                return Ok(l1 == l2 && h1 == h2);
            }
            (SetRepr::Subset(x), SetRepr::Subset(y)) => return equals(x, y, ctx),
            (SetRepr::FcnSet(d1, r1), SetRepr::FcnSet(d2, r2)) => {
                let (u1, u2) = (is_empty(d1, ctx)?, is_empty(d2, ctx)?);
                if u1 || u2 {
                    return Ok(u1 == u2);
                }
                let (e1, e2) = (is_empty(r1, ctx)?, is_empty(r2, ctx)?);
                if e1 || e2 {
                    return Ok(e1 == e2);
                }
                return Ok(equals(d1, d2, ctx)? && equals(r1, r2, ctx)?);
            }
            (SetRepr::RcdSet(f1), SetRepr::RcdSet(f2)) => {
                if is_empty(a, ctx)? {
                    return is_empty(b, ctx);
                }
                if is_empty(b, ctx)? {
                    return Ok(false);
                }
                if f1.len() != f2.len() {
                    return Ok(false);
                }
                for ((n1, s1), (n2, s2)) in f1.iter().zip(f2.iter()) {
                    if n1 != n2 || !equals(s1, s2, ctx)? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            (SetRepr::Product(f1), SetRepr::Product(f2)) => {
                if is_empty(a, ctx)? {
                    return is_empty(b, ctx);
                }
                if is_empty(b, ctx)? {
                    return Ok(false);
                }
                if f1.len() != f2.len() {
                    return Ok(false);
                }
                for (s1, s2) in f1.iter().zip(f2.iter()) {
                    if !equals(s1, s2, ctx)? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            _ => {}
        }
    }
    let mine = s.elements(ctx)?;
    let Some(theirs) = to_set_enum(b, ctx)? else {
        if let Value::Model(m) = b {
            return mv_equals_rev(m, b, a, ctx);
        }
        return Err(fail(format!(
            "Attempted to check equality of the set {} with the value:\n{}",
            p(&enum_value(&mine, ctx), ctx),
            p(b, ctx)
        )));
    };
    if mine.len() != theirs.len() {
        return Ok(false);
    }
    for (x, y) in mine.iter().zip(theirs.iter()) {
        if !equals(x, y, ctx)? {
            return Ok(false);
        }
    }
    Ok(true)
}

fn equals_fcn(f: &Fcn, a: &Value, b: &Value, ctx: Ctx) -> Result<bool> {
    match &f.0.repr {
        FcnRepr::Tuple(e) => {
            if let Some(t) = to_tuple(b) {
                if e.len() != t.len() {
                    return Ok(false);
                }
                for (x, y) in e.iter().zip(t) {
                    if !equals(x, y, ctx)? {
                        return Ok(false);
                    }
                }
                return Ok(true);
            }
            equals_fcnrcd(&FcnView::Intv(e), a, b, ctx)
        }
        FcnRepr::Record(fs) => {
            let Some(r) = to_rcd(b) else {
                if let Value::Model(m) = b {
                    return mv_equals_rev(m, b, a, ctx);
                }
                return Err(fail(format!(
                    "Attempted to check equality of record:\n{}\nwith non-record\n{}",
                    p(a, ctx),
                    p(b, ctx)
                )));
            };
            if fs.len() != r.len() {
                return Ok(false);
            }
            if fs.iter().zip(r).any(|((n1, _), (n2, _))| n1 != n2) {
                return Ok(false);
            }
            for ((_, v1), (_, v2)) in fs.iter().zip(r) {
                if !equals(v1, v2, ctx)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        FcnRepr::General(_) => {
            let view = FcnView::Array(f.iter().collect());
            equals_fcnrcd(&view, a, b, ctx)
        }
    }
}

fn equals_fcnrcd(me: &FcnView<'_>, a: &Value, b: &Value, ctx: Ctx) -> Result<bool> {
    let Some(other) = to_fcn(b) else {
        if let Value::Model(m) = b {
            return mv_equals_rev(m, b, a, ctx);
        }
        return Err(fail(format!(
            "Attempted to check equality of the function {} with the value:\n{}",
            p(a, ctx),
            p(b, ctx)
        )));
    };
    if me.len() != other.len() {
        return Ok(false);
    }
    match (me, &other) {
        (FcnView::Intv(_), FcnView::Intv(_)) => {}
        (FcnView::Intv(_), FcnView::Array(ps)) | (FcnView::Array(ps), FcnView::Intv(_)) => {
            for (i, (d, _)) in ps.iter().enumerate() {
                let Value::Int(k) = d else {
                    return Err(non_int_domain(d, ctx, true));
                };
                if *k != idx_value(i) {
                    return Ok(false);
                }
            }
        }
        (FcnView::Array(p1), FcnView::Array(p2)) => {
            for ((d1, _), (d2, _)) in p1.iter().zip(p2) {
                if !equals(d1, d2, ctx)? {
                    return Ok(false);
                }
            }
        }
    }
    for i in 0..me.len() {
        if !equals(me.value(i), other.value(i), ctx)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// TLC's `Value.isEmpty()`.
///
/// # Errors
/// For a non-set (`Shouldn't call isEmpty() on value ...`), and enumeration errors.
pub fn is_empty(v: &Value, ctx: Ctx) -> Result<bool> {
    let Value::Set(s) = v else {
        return Err(fail(format!(
            "Shouldn't call isEmpty() on value {}",
            p(v, ctx)
        )));
    };
    Ok(match s.repr() {
        SetRepr::Interval(..) | SetRepr::Enum | SetRepr::Raw(_) | SetRepr::Deferred(_) => {
            s.elements(ctx)?.is_empty()
        }
        SetRepr::FcnSet(d, r) => !is_empty(d, ctx)? && is_empty(r, ctx)?,
        SetRepr::RcdSet(fs) => {
            let mut any = false;
            for (_, f) in fs {
                any |= is_empty(f, ctx)?;
            }
            any
        }
        SetRepr::Product(fs) => {
            let mut any = false;
            for f in fs {
                any |= is_empty(f, ctx)?;
            }
            any
        }
        SetRepr::Subset(_) | SetRepr::Nat | SetRepr::Int | SetRepr::Strings | SetRepr::Seq(_) => {
            false
        }
    })
}

/// TLC's `set.member(elem)`: `elem \in set` as TLC evaluates it, without enumerating
/// lazy sets.
///
/// # Errors
/// The error TLC raises, such as
/// `Attempted to check if the value:\n"a"\nis in the integer interval 1..3`.
#[allow(
    clippy::too_many_lines,
    reason = "one arm per TLC member() method, kept together for comparison"
)]
pub fn member(elem: &Value, set: &Value, ctx: Ctx) -> Result<bool> {
    let s = match set {
        Value::Set(s) => s,
        Value::Fcn(f) => {
            return Err(fail_member(match &f.0.repr {
                FcnRepr::Tuple(_) => {
                    "Attempted to check set membership in a tuple value.".to_owned()
                }
                FcnRepr::Record(_) => format!(
                    "Attempted to check if element:\n{}\nis in the record:\n{}",
                    p(elem, ctx),
                    p(set, ctx)
                ),
                FcnRepr::General(_) => format!(
                    "Attempted to check if the value:\n{}\nis an element of the function {}",
                    p(elem, ctx),
                    p(set, ctx)
                ),
            }));
        }
        _ => {
            let what = match set {
                Value::Bool(_) => "boolean",
                Value::Int(_) | Value::BigInt(_) => "integer",
                Value::Str(_) => "string",
                _ => "model value",
            };
            return Err(fail_member(format!(
                "Attempted to check if the value:\n{}\nis an element of the {what} {}",
                p(elem, ctx),
                p(set, ctx)
            )));
        }
    };
    match s.repr() {
        SetRepr::Enum => binary_search(elem, &s.elements(ctx)?, ctx),
        SetRepr::Raw(raw) => {
            if let Some(Ok(sorted)) = s.0.elems.get() {
                binary_search(elem, sorted, ctx)
            } else {
                for x in raw {
                    if equals(x, elem, ctx)? {
                        return Ok(true);
                    }
                }
                Ok(false)
            }
        }
        SetRepr::Interval(lo, hi) => {
            if let Some(x) = elem.as_int() {
                return Ok(&x >= lo && &x <= hi);
            }
            let typed_or_not_mv = match elem {
                Value::Model(m) => m.mv_type().is_some(),
                _ => true,
            };
            if lo <= hi && typed_or_not_mv {
                return Err(fail_member(format!(
                    "Attempted to check if the value:\n{}\nis in the integer interval {}",
                    p(elem, ctx),
                    p(set, ctx)
                )));
            }
            Ok(false)
        }
        SetRepr::Subset(base) => match elem {
            Value::Set(e) if user_of(e).is_none() => {
                for x in e.elements(ctx)?.iter() {
                    if !member(x, base, ctx)? {
                        return Ok(false);
                    }
                }
                Ok(true)
            }
            _ => Err(fail_member(format!(
                "Attempted to check if the non-enumerable value\n{}\nis element of\n{}",
                p(elem, ctx),
                p(set, ctx)
            ))),
        },
        SetRepr::FcnSet(domain, range) => member_fcn_set(elem, set, domain, range, ctx),
        SetRepr::RcdSet(fs) => {
            let Some(r) = to_rcd(elem) else {
                if let Value::Model(m) = elem {
                    return mv_member(m, elem, set, ctx);
                }
                return Err(fail_member(format!(
                    "Attempted to check if non-record\n{}\nis in the set of records:\n{}",
                    to_tlc_string_lenient(elem, ctx),
                    p(set, ctx)
                )));
            };
            if fs.len() != r.len() {
                return Ok(false);
            }
            for ((n1, s1), (n2, v)) in fs.iter().zip(r) {
                if n1 != n2 || !member(v, s1, ctx)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        SetRepr::Product(fs) => {
            let Some(t) = to_tuple(elem) else {
                let non_tuple = || {
                    fail_member(format!(
                        "Attempted to check if non-tuple\n{}\nis in the set of tuples:\n{}",
                        p(elem, ctx),
                        p(set, ctx)
                    ))
                };
                return match elem {
                    Value::Model(m) => mv_member(m, elem, set, ctx),
                    Value::Fcn(f) => {
                        if f.iter().all(|(d, _)| matches!(d, Value::Int(_))) {
                            Ok(false)
                        } else {
                            Err(non_tuple())
                        }
                    }
                    _ => Err(non_tuple()),
                };
            };
            if t.len() != fs.len() {
                return Ok(false);
            }
            for (x, f) in t.iter().zip(fs.iter()) {
                if !member(x, f, ctx)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        SetRepr::Nat | SetRepr::Int | SetRepr::Strings => {
            let (ok, name) = match s.repr() {
                SetRepr::Nat => (elem.as_int().map(|i| !i.is_negative()), "Nat"),
                SetRepr::Int => (elem.as_int().map(|_| true), "Int"),
                _ => (matches!(elem, Value::Str(_)).then_some(true), "STRING"),
            };
            if let Some(ok) = ok {
                return Ok(ok);
            }
            if let Value::Model(m) = elem {
                return mv_member(m, elem, set, ctx);
            }
            Err(ValueError::new(
                ErrorKind::ModuleMembership,
                format!(
                    "Attempted to check if the value:\n{}\nis an element of {name}.",
                    p(elem, ctx)
                ),
            ))
        }
        SetRepr::Seq(range) => {
            let Some(t) = to_tuple(elem) else {
                if let Value::Model(m) = elem {
                    return mv_member(m, elem, set, ctx);
                }
                return Err(ValueError::new(
                    ErrorKind::ModuleMembership,
                    format!(
                        "Attempted to check if the value:\n{}\nis an element of {}.",
                        p(elem, ctx),
                        p(set, ctx)
                    ),
                ));
            };
            for x in t {
                if !member(x, range, ctx)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        SetRepr::Deferred(src) => match src.member(elem, ctx) {
            Some(r) => r,
            None => binary_search(elem, &s.elements(ctx)?, ctx),
        },
    }
}

fn member_fcn_set(
    elem: &Value,
    set: &Value,
    domain: &Value,
    range: &Value,
    ctx: Ctx,
) -> Result<bool> {
    let Some(f) = to_fcn(elem) else {
        if let Value::Model(m) = elem {
            return mv_member(m, elem, set, ctx);
        }
        return Err(fail_member(format!(
            "Attempted to check if \n{}\nwhich is not a TLC function value, is in the set of functions:\n{}",
            to_tlc_string_lenient(elem, ctx),
            p(set, ctx)
        )));
    };
    let dom_ok = match &f {
        FcnView::Intv(e) => {
            // `IntervalValue(1, n).equals(domain)`.
            let n = i64::try_from(e.len()).unwrap_or(i64::MAX);
            let intv = Value::interval(crate::Int::ONE, crate::Int::Small(n), ctx);
            equals(&intv, domain, ctx)?
        }
        FcnView::Array(ps) => {
            // `this.domain.equals(new SetEnumValue(fcn.domain, true))`.
            let dom = Value::Set(Set::from_sorted(
                ctx.order,
                ps.iter().map(|(d, _)| d.clone()).collect(),
            ));
            equals(domain, &dom, ctx)?
        }
    };
    if !dom_ok {
        return Ok(false);
    }
    for i in 0..f.len() {
        if !member(f.value(i), range, ctx)? {
            return Ok(false);
        }
    }
    Ok(true)
}

/// `ValueVec.search(elem, true)`: binary search with `elem.compareTo(sorted[mid])`.
fn binary_search(elem: &Value, sorted: &[Value], ctx: Ctx) -> Result<bool> {
    let (mut low, mut high) = (0usize, sorted.len());
    while low < high {
        let mid = low + ((high - low) >> 1);
        match compare(elem, &sorted[mid], ctx)? {
            Ordering::Equal => return Ok(true),
            Ordering::Less => high = mid,
            Ordering::Greater => low = mid + 1,
        }
    }
    Ok(false)
}
