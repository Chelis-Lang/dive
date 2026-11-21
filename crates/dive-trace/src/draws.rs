//! Which values a random operator can return, decided without enumerating
//! its sets (ADR 0162 §8).
//!
//! A recorded draw is used as an operator's result only when [`admits`]
//! shows, by this validator's own evaluation of the arguments, that the
//! operator can return it:
//! - `RandomElement(S)`: `S` is a finite set and the value is a member of
//!   `S`, checked with TLC's membership test, which does not enumerate a
//!   lazily held set such as `[D -> R]`.
//!
//! Anything this cannot decide (an argument of the wrong kind, an infinite
//! set, an error while testing membership) admits nothing, which leaves the
//! call to the bounded enumeration or to a refusal.

use dive_ir::StdOp;
use dive_value::{Ctx, SetKind, Value};

/// Whether `op` applied to `args` can return `value`.
pub(crate) fn admits(op: StdOp, args: &[Value], value: &Value, ctx: Ctx) -> bool {
    match (op, args) {
        (StdOp::TlcRandomElement, [set]) => finite_set(set) && member(value, set, ctx),
        _ => false,
    }
}

/// A set the random operators accept: finite, as TLC's `toSetEnum` needs.
fn finite_set(value: &Value) -> bool {
    match value {
        Value::Set(set) => {
            !matches!(set.kind(), SetKind::Nat | SetKind::Int | SetKind::Strings)
                && !set.is_infinite()
                && set.kind() != SetKind::Deferred
        }
        _ => false,
    }
}

/// `value \in set` by TLC's membership test; an error admits nothing.
fn member(value: &Value, set: &Value, ctx: Ctx) -> bool {
    matches!(dive_value::tlc::member(value, set, ctx), Ok(true))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval(lo: i64, hi: i64, ctx: Ctx) -> Value {
        Value::interval(lo.into(), hi.into(), ctx)
    }

    #[test]
    fn random_element_admits_exactly_the_members_of_its_set() {
        let ctx = Ctx::dive();
        let set = interval(1, 3, ctx);
        let admits = |v: i64| {
            admits(
                StdOp::TlcRandomElement,
                std::slice::from_ref(&set),
                &Value::Int(v),
                ctx,
            )
        };
        assert!(admits(1) && admits(3));
        assert!(!admits(0) && !admits(4), "a draw outside its set");
        assert!(!admits_infinite(ctx), "an infinite set admits nothing");
    }

    fn admits_infinite(ctx: Ctx) -> bool {
        admits(
            StdOp::TlcRandomElement,
            &[Value::nat(ctx)],
            &Value::Int(1),
            ctx,
        )
    }
}
