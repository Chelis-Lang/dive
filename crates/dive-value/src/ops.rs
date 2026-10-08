//! Set algebra on finite sets, as TLC evaluates it.
//!
//! These port TLC's eager forms (`SetEnumValue.cup`, `cap`, `diff`; tlaplus
//! `94d0c50`), including which results stay unnormalized: a union is built
//! unnormalized, and an intersection or difference keeps its left operand's state. That
//! matters for TLC's errors, because membership in an unnormalized set is a linear scan
//! with `equals` (fixture `ErrMemNormalized`). Membership uses the checked
//! [`crate::tlc::member`]. Lazy unions of infinite sets are `dive-eval`'s concern
//! ([`DeferredSet`](crate::DeferredSet)).

use crate::ctx::Ctx;
use crate::error::{ErrorKind, Result, ValueError};
use crate::tlc::member;
use crate::value::{Set, SetRepr, Value};

/// The elements of a set in TLC's current order, and whether that order is normalized.
fn current(v: &Value, ctx: Ctx) -> Result<(Vec<Value>, bool)> {
    match v {
        Value::Set(s) => match s.repr() {
            SetRepr::Raw(raw) => match s.0.elems.get() {
                Some(Ok(sorted)) => Ok((sorted.to_vec(), true)),
                _ => Ok((raw.to_vec(), false)),
            },
            _ => Ok((s.elements(ctx)?.to_vec(), true)),
        },
        _ => Err(ValueError::new(
            ErrorKind::Argument,
            format!(
                "Attempted to apply a set operator to the non-set value\n{}",
                crate::print::print_value(v, ctx).unwrap_or_default()
            ),
        )),
    }
}

fn build(elems: Vec<Value>, normalized: bool, ctx: Ctx) -> Value {
    if normalized {
        Value::Set(Set::from_sorted(ctx.order, elems))
    } else {
        Value::set_raw(elems, ctx)
    }
}

/// `a \cup b` (`SetEnumValue.cup`): `a`'s elements, then `b`'s elements not in `a`,
/// unnormalized.
///
/// # Errors
/// Enumeration and membership errors.
pub fn union(a: &Value, b: &Value, ctx: Ctx) -> Result<Value> {
    let (mut out, _) = current(a, ctx)?;
    if out.is_empty() {
        return Ok(b.clone());
    }
    // TLC enumerates `b` with `elements()`, which normalizes it.
    let Value::Set(bs) = b else {
        return current(b, ctx).map(|(_, _)| b.clone());
    };
    for e in bs.elements(ctx)?.iter() {
        if !member(e, a, ctx)? {
            out.push(e.clone());
        }
    }
    Ok(Value::set_raw(out, ctx))
}

/// `a \cap b` (`SetEnumValue.cap`).
///
/// # Errors
/// Enumeration and membership errors.
pub fn intersect(a: &Value, b: &Value, ctx: Ctx) -> Result<Value> {
    let (elems, norm) = current(a, ctx)?;
    let mut out = Vec::new();
    for e in elems {
        if member(&e, b, ctx)? {
            out.push(e);
        }
    }
    Ok(build(out, norm, ctx))
}

/// `a \ b` (`SetEnumValue.diff`).
///
/// # Errors
/// Enumeration and membership errors.
pub fn difference(a: &Value, b: &Value, ctx: Ctx) -> Result<Value> {
    let (elems, norm) = current(a, ctx)?;
    let mut out = Vec::new();
    for e in elems {
        if !member(&e, b, ctx)? {
            out.push(e);
        }
    }
    Ok(build(out, norm, ctx))
}

/// The first element of `s` in normalized order that satisfies `pred`: TLC's
/// `CHOOSE x \in s : pred(x)` (`Tool`, `OPCODE_bc`, which enumerates with
/// `Ordering.NORMALIZED`). `None` when no element satisfies it.
///
/// # Errors
/// Enumeration errors and errors from `pred`.
pub fn choose(
    s: &Value,
    ctx: Ctx,
    mut pred: impl FnMut(&Value) -> Result<bool>,
) -> Result<Option<Value>> {
    let Value::Set(set) = s else {
        return Err(ValueError::new(
            ErrorKind::NotEnumerable,
            "Attempted to compute the value of an expression of form CHOOSE x \\in S: P, but S was not enumerable.",
        ));
    };
    for e in set.elements(ctx)?.iter() {
        if pred(e)? {
            return Ok(Some(e.clone()));
        }
    }
    Ok(None)
}
