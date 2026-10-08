//! Set algebra on finite sets.
//!
//! These follow TLC's eager forms (`SetEnumValue.cup`, `cap`, `diff`): the result is
//! an enumerated, normalized set, and membership uses TLC's checked
//! [`member`](crate::tlc::member). Lazy unions of infinite sets are `dive-eval`'s
//! concern ([`DeferredSet`](crate::DeferredSet)).

use crate::ctx::Ctx;
use crate::error::{ErrorKind, Result, ValueError};
use crate::tlc::member;
use crate::value::{Set, Value};

fn elems(v: &Value, ctx: Ctx) -> Result<std::sync::Arc<[Value]>> {
    match v {
        Value::Set(s) => s.elements(ctx),
        _ => Err(ValueError::new(
            ErrorKind::Argument,
            format!(
                "Attempted to apply a set operator to the non-set value\n{}",
                crate::print::print_value(v, ctx).unwrap_or_default()
            ),
        )),
    }
}

/// `a \cup b`.
///
/// # Errors
/// Enumeration and comparison errors.
pub fn union(a: &Value, b: &Value, ctx: Ctx) -> Result<Value> {
    let mut all: Vec<Value> = elems(a, ctx)?.to_vec();
    all.extend(elems(b, ctx)?.iter().cloned());
    Value::set(all, ctx)
}

/// `a \cap b`.
///
/// # Errors
/// Enumeration and membership errors.
pub fn intersect(a: &Value, b: &Value, ctx: Ctx) -> Result<Value> {
    let mut out = Vec::new();
    for e in elems(a, ctx)?.iter() {
        if member(e, b, ctx)? {
            out.push(e.clone());
        }
    }
    Ok(Value::Set(Set::from_sorted(ctx.order, out)))
}

/// `a \ b`.
///
/// # Errors
/// Enumeration and membership errors.
pub fn difference(a: &Value, b: &Value, ctx: Ctx) -> Result<Value> {
    let mut out = Vec::new();
    for e in elems(a, ctx)?.iter() {
        if !member(e, b, ctx)? {
            out.push(e.clone());
        }
    }
    Ok(Value::Set(Set::from_sorted(ctx.order, out)))
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
    for e in elems(s, ctx)?.iter() {
        if pred(e)? {
            return Ok(Some(e.clone()));
        }
    }
    Ok(None)
}
