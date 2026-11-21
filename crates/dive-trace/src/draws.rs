//! Which values a random operator can return, decided without enumerating
//! its sets (ADR 0162 §8).
//!
//! A recorded draw is used as an operator's result only when [`admits`]
//! shows, by this validator's own evaluation of the arguments, that the
//! operator can return it:
//! - `RandomElement(S)`: `S` is a finite set and the value is a member of
//!   `S`, checked with TLC's membership test, which does not enumerate a
//!   lazily held set such as `[D -> R]`.
//! - `RandomSubset(k, S)`: `k` is a nonnegative integer, `S` a finite set,
//!   and the value a set of `min(k, Cardinality(S))` members of `S` (TLC's
//!   override returns all of `S` when `k` exceeds its size). Only the drawn
//!   set is enumerated, never `S`.
//! - `RandomSetOfSubsets(k, n, S)`: `k` and `n` are nonnegative integers
//!   within TLC's bounds, `S` a finite set, and the value a set of between
//!   1 and `k` subsets of `S` (none for `k = 0`), each empty when `n = 0`
//!   and all of `S` when `n = Cardinality(S)`, since each element is kept
//!   with probability `n / Cardinality(S)`.
//!
//! Anything this cannot decide (an argument of the wrong kind, an infinite
//! set, an error while testing membership or size) admits nothing, which
//! leaves the call to the bounded enumeration or to a refusal.
//!
//! Each relation is exact: a value it admits is one the operator can return
//! and every other value is not. [`decide`] and [`decide_member`] therefore
//! settle `v = Op(..)` and `v \in Op(..)` for one direct `RandomSubset` or
//! `RandomSetOfSubsets` application, whose
//! draw is used nowhere else, without drawing; for arguments TLC rejects
//! they decide nothing, so the operator's error is still reached.

use dive_ir::StdOp;
use dive_value::{Ctx, SetKind, Value};

/// Whether `op` applied to `args` can return `value`.
pub(crate) fn admits(op: StdOp, args: &[Value], value: &Value, ctx: Ctx) -> bool {
    decide(op, args, value, ctx) == Some(true)
}

/// Whether `op` applied to `args` can return `value`; `None` when the
/// arguments are outside the operator's domain (TLC raises an error).
pub(crate) fn decide(op: StdOp, args: &[Value], value: &Value, ctx: Ctx) -> Option<bool> {
    match (op, args) {
        (StdOp::TlcRandomElement, [set]) => (size(set, ctx)? > 0).then(|| member(value, set, ctx)),
        (StdOp::RandomizationRandomSubset, [k, set]) => {
            let (k, n) = (count(k)?, size(set, ctx)?);
            Some(random_subset(k.min(n), set, value, ctx))
        }
        (StdOp::RandomizationRandomSetOfSubsets, [k, n, set]) => {
            let bounds = set_of_subsets_bounds(k, n, set, ctx)?;
            Some(random_set_of_subsets(bounds, set, value, ctx))
        }
        _ => None,
    }
}

/// Whether `value` is a member of some value `op` applied to `args` can
/// return; `None` when the arguments are outside the operator's domain or
/// this is not decided without enumeration (`RandomElement`).
pub(crate) fn decide_member(op: StdOp, args: &[Value], value: &Value, ctx: Ctx) -> Option<bool> {
    match (op, args) {
        (StdOp::RandomizationRandomSubset, [k, set]) => {
            let (k, n) = (count(k)?, size(set, ctx)?);
            Some(k.min(n) > 0 && member(value, set, ctx))
        }
        (StdOp::RandomizationRandomSetOfSubsets, [k, n, set]) => {
            let bounds = set_of_subsets_bounds(k, n, set, ctx)?;
            Some(bounds.0 > 0 && subset(bounds, set, value, ctx))
        }
        _ => None,
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

/// A nonnegative integer argument.
fn count(value: &Value) -> Option<u64> {
    value.as_int()?.to_i64().and_then(|v| u64::try_from(v).ok())
}

/// `Cardinality(set)` of a finite set, from its structure where it has one.
fn size(set: &Value, ctx: Ctx) -> Option<u64> {
    match set {
        Value::Set(s) if finite_set(set) => s.size(ctx).ok(),
        _ => None,
    }
}

/// The elements of a drawn set.
fn drawn(value: &Value, ctx: Ctx) -> Option<std::sync::Arc<[Value]>> {
    match value {
        Value::Set(s) if finite_set(value) => s.elements(ctx).ok(),
        _ => None,
    }
}

/// `value` is a set of exactly `len` members of `set`.
fn random_subset(len: u64, set: &Value, value: &Value, ctx: Ctx) -> bool {
    drawn(value, ctx).is_some_and(|elements| {
        elements.len() as u64 == len && elements.iter().all(|e| member(e, set, ctx))
    })
}

/// `(k, n, |S|)` when TLC accepts `RandomSetOfSubsets(k, n, S)`: `n <=
/// |S|`, and `k <= 2^|S|` once `k`'s bit length exceeds `|S|`.
fn set_of_subsets_bounds(k: &Value, n: &Value, set: &Value, ctx: Ctx) -> Option<(u64, u64, u64)> {
    let (k, n, size) = (count(k)?, count(n)?, size(set, ctx)?);
    let too_many = size < 63 && u64::from(64 - k.leading_zeros()) > size && k > 1u64 << size;
    (n <= size && !too_many).then_some((k, n, size))
}

/// `value` is a subset of `set` that keeping each element with probability
/// `n / |S|` can give: empty when `n = 0`, all of `set` when `n = |S|`.
fn subset((_, n, size): (u64, u64, u64), set: &Value, value: &Value, ctx: Ctx) -> bool {
    drawn(value, ctx).is_some_and(|members| {
        let len = members.len() as u64;
        (n > 0 || len == 0)
            && (n < size || len == size)
            && members.iter().all(|e| member(e, set, ctx))
    })
}

/// `value` is a set of between 1 and `k` such subsets (none for `k = 0`).
fn random_set_of_subsets(bounds: (u64, u64, u64), set: &Value, value: &Value, ctx: Ctx) -> bool {
    let k = bounds.0;
    drawn(value, ctx).is_some_and(|subsets| {
        let picks = subsets.len() as u64;
        picks <= k && (k == 0 || picks > 0) && subsets.iter().all(|s| subset(bounds, set, s, ctx))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn interval(lo: i64, hi: i64, ctx: Ctx) -> Value {
        Value::interval(lo.into(), hi.into(), ctx)
    }

    fn set(values: &[i64], ctx: Ctx) -> Value {
        Value::set_raw(values.iter().copied().map(Value::Int).collect(), ctx)
    }

    #[test]
    fn random_element_admits_exactly_the_members_of_its_set() {
        let ctx = Ctx::dive();
        let s = interval(1, 3, ctx);
        let admits_value = |v: i64| {
            admits(
                StdOp::TlcRandomElement,
                std::slice::from_ref(&s),
                &Value::Int(v),
                ctx,
            )
        };
        assert!(admits_value(1) && admits_value(3));
        assert!(
            !admits_value(0) && !admits_value(4),
            "a draw outside its set"
        );
        assert!(
            !admits(
                StdOp::TlcRandomElement,
                &[Value::nat(ctx)],
                &Value::Int(1),
                ctx
            ),
            "an infinite set admits nothing"
        );
    }

    #[test]
    fn random_subset_admits_sets_of_the_drawn_size_inside_its_set() {
        let ctx = Ctx::dive();
        let s = interval(1, 10, ctx);
        let admits_subset = |k: i64, value: &[i64]| {
            admits(
                StdOp::RandomizationRandomSubset,
                &[Value::Int(k), s.clone()],
                &set(value, ctx),
                ctx,
            )
        };
        assert!(admits_subset(2, &[5, 6]));
        assert!(!admits_subset(2, &[5]), "the wrong size");
        assert!(!admits_subset(2, &[5, 6, 7]), "the wrong size");
        assert!(!admits_subset(2, &[5, 11]), "a member outside the set");
        assert!(admits_subset(0, &[]));
        assert!(
            admits_subset(12, &(1..=10).collect::<Vec<_>>()),
            "k above |S| is all of S"
        );
        assert!(!admits_subset(12, &[1, 2]));
        assert!(
            !admits_subset(-1, &[]),
            "a negative count is TLC's error, never a draw"
        );
    }

    #[test]
    fn random_subset_of_a_function_set_never_enumerates_it() -> Result<(), dive_value::ValueError> {
        // [1..40 -> 0..1] has 2^40 members: size and membership are structural.
        let ctx = Ctx::dive();
        let functions = Value::fcn_set(interval(1, 40, ctx), interval(0, 1, ctx), ctx);
        let constant = |f: fn(i64) -> i64| {
            Value::fcn(
                (1..=40)
                    .map(|i| (Value::Int(i), Value::Int(f(i))))
                    .collect(),
                ctx,
            )
        };
        let drawn = Value::set_raw(vec![constant(|_| 0)?, constant(|i| i % 2)?], ctx);
        let subset = |k: i64| {
            admits(
                StdOp::RandomizationRandomSubset,
                &[Value::Int(k), functions.clone()],
                &drawn,
                ctx,
            )
        };
        assert!(subset(2));
        assert!(!subset(3));
        Ok(())
    }

    #[test]
    fn arguments_tlc_rejects_decide_nothing() {
        let ctx = Ctx::dive();
        let s = interval(1, 5, ctx);
        let any = set(&[], ctx);
        let subset = StdOp::RandomizationRandomSubset;
        let subsets = StdOp::RandomizationRandomSetOfSubsets;
        assert_eq!(
            decide(subset, &[Value::Int(-1), s.clone()], &any, ctx),
            None
        );
        assert_eq!(
            decide_member(subset, &[Value::Int(-1), s.clone()], &any, ctx),
            None
        );
        assert_eq!(
            decide(
                subsets,
                &[Value::Int(1), Value::Int(6), s.clone()],
                &any,
                ctx
            ),
            None,
            "n above |S|"
        );
        assert_eq!(
            decide(StdOp::TlcRandomElement, &[set(&[], ctx)], &any, ctx),
            None,
            "an empty set"
        );
        assert_eq!(
            decide(StdOp::TlcRandomElement, &[s], &Value::Int(9), ctx),
            Some(false)
        );
    }

    #[test]
    fn membership_in_some_draw_is_decided_from_the_set() {
        let ctx = Ctx::dive();
        let s = interval(1, 5, ctx);
        let subset = |k: i64, v: Value| {
            decide_member(
                StdOp::RandomizationRandomSubset,
                &[Value::Int(k), s.clone()],
                &v,
                ctx,
            )
        };
        assert_eq!(subset(2, Value::Int(3)), Some(true));
        assert_eq!(subset(2, Value::Int(6)), Some(false));
        assert_eq!(subset(0, Value::Int(3)), Some(false), "k = 0 draws {{}}");
        let subsets = |k: i64, n: i64, v: &[i64]| {
            decide_member(
                StdOp::RandomizationRandomSetOfSubsets,
                &[Value::Int(k), Value::Int(n), s.clone()],
                &set(v, ctx),
                ctx,
            )
        };
        assert_eq!(subsets(2, 2, &[1, 4]), Some(true));
        assert_eq!(subsets(2, 2, &[1, 6]), Some(false));
        assert_eq!(subsets(2, 0, &[1]), Some(false));
        assert_eq!(subsets(2, 0, &[]), Some(true));
        assert_eq!(subsets(0, 2, &[]), Some(false));
    }

    #[test]
    fn random_set_of_subsets_admits_subsets_as_k_and_n_allow() {
        let ctx = Ctx::dive();
        let s = interval(1, 5, ctx);
        let admits_set = |k: i64, n: i64, value: Vec<Value>| {
            admits(
                StdOp::RandomizationRandomSetOfSubsets,
                &[Value::Int(k), Value::Int(n), s.clone()],
                &Value::set_raw(value, ctx),
                ctx,
            )
        };
        assert!(admits_set(
            3,
            2,
            vec![set(&[1], ctx), set(&[], ctx), set(&[2, 5], ctx)]
        ));
        assert!(
            !admits_set(2, 2, vec![set(&[1], ctx), set(&[], ctx), set(&[2], ctx)]),
            "more than k"
        );
        assert!(!admits_set(3, 2, vec![]), "at least one subset when k > 0");
        assert!(admits_set(0, 2, vec![]));
        assert!(!admits_set(3, 2, vec![set(&[6], ctx)]), "not a subset");
        assert!(admits_set(3, 0, vec![set(&[], ctx)]));
        assert!(
            !admits_set(3, 0, vec![set(&[1], ctx)]),
            "n = 0 keeps nothing"
        );
        assert!(admits_set(3, 5, vec![set(&[1, 2, 3, 4, 5], ctx)]));
        assert!(
            !admits_set(3, 5, vec![set(&[1], ctx)]),
            "n = |S| keeps everything"
        );
        assert!(
            !admits_set(3, 6, vec![set(&[1], ctx)]),
            "n above |S| is TLC's error"
        );
    }
}
