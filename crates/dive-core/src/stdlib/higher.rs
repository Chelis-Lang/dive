//! Operators with operator arguments: `Sequences.SelectSeq`, `TLC.SortSeq`,
//! `Bags.BagOfAll`, `BagsExt.FoldBag`, `FiniteSetsExt.Quantify`,
//! `Functions.FoldFunction` and `SequencesExt`'s folds and selections. The
//! reference is the reference engine's `std.rs` and `community.rs`
//! (`bag_of_all`, `fold_bag`, `select_in`, `fold_function`, `test_bool`).
//!
//! Each operator applies its operator argument in an order fixed by a
//! sequence, or by the normalized order of a function or set where the
//! result does not depend on it. Folds over sets and unnormalized
//! functions, whose order is TLC's internal order, are not here.

use dive_ir::StdOp;

use super::bags;
use super::{Ho, OpFn, arg_error, enumerable, exc, refuse};
use crate::eval::Machine;
use crate::node::Kind;
use crate::{Error, Handle};

pub(super) fn implements(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::SequencesSelectSeq
            | StdOp::TlcSortSeq
            | StdOp::BagsBagOfAll
            | StdOp::BagsExtFoldBag
            | StdOp::FiniteSetsExtQuantify
            | StdOp::FunctionsFoldFunction
            | StdOp::SequencesExtFoldSeq
            | StdOp::SequencesExtFoldLeft
            | StdOp::SequencesExtFoldRight
            | StdOp::SequencesExtFoldLeftDomain
            | StdOp::SequencesExtFoldRightDomain
            | StdOp::SequencesExtSelectInSeq
            | StdOp::SequencesExtSelectLastInSeq
            | StdOp::SequencesExtRemoveFirstMatch
    )
}

/// Whether TLC's override takes the operator as an `OpValue`. Passed an
/// operator whose body is a constant such as `LAMBDA x : 0`, TLC fails
/// with error 2154 or 2266 while the reference engine applies it
/// (`04-plan-compiler.md` §8, fixtures `ErHoConst*`); the core refuses
/// such a call.
pub(super) fn takes_op_value(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::BagsBagOfAll
            | StdOp::BagsExtFoldBag
            | StdOp::FunctionsFoldFunction
            | StdOp::SequencesExtFoldSeq
            | StdOp::SequencesExtFoldLeft
            | StdOp::SequencesExtFoldRight
            | StdOp::SequencesExtFoldLeftDomain
            | StdOp::SequencesExtFoldRightDomain
    )
}

pub(super) fn op_arity(op: StdOp, i: usize) -> Option<usize> {
    match (op, i) {
        (StdOp::BagsBagOfAll, 0)
        | (
            StdOp::SequencesSelectSeq
            | StdOp::FiniteSetsExtQuantify
            | StdOp::SequencesExtSelectInSeq
            | StdOp::SequencesExtSelectLastInSeq
            | StdOp::SequencesExtRemoveFirstMatch,
            1,
        ) => Some(1),
        (StdOp::TlcSortSeq, 1)
        | (
            StdOp::BagsExtFoldBag
            | StdOp::FunctionsFoldFunction
            | StdOp::SequencesExtFoldSeq
            | StdOp::SequencesExtFoldLeft
            | StdOp::SequencesExtFoldRight
            | StdOp::SequencesExtFoldLeftDomain
            | StdOp::SequencesExtFoldRightDomain,
            0,
        ) => Some(2),
        _ => None,
    }
}

pub(super) fn call(m: &mut Machine, op: StdOp, args: &[Ho<'_>]) -> Result<Handle, Error> {
    match op {
        StdOp::SequencesSelectSeq => select_seq(m, op, value(args, 0)?, func(args, 1)?),
        StdOp::TlcSortSeq => sort_seq(m, op, value(args, 0)?, func(args, 1)?),
        StdOp::BagsBagOfAll => bag_of_all(m, op, func(args, 0)?, value(args, 1)?),
        StdOp::BagsExtFoldBag => fold_bag(m, op, func(args, 0)?, value(args, 1)?, value(args, 2)?),
        StdOp::FiniteSetsExtQuantify => quantify(m, op, value(args, 0)?, func(args, 1)?),
        StdOp::FunctionsFoldFunction | StdOp::SequencesExtFoldSeq => {
            fold_function(m, op, func(args, 0)?, value(args, 1)?, value(args, 2)?)
        }
        StdOp::SequencesExtFoldLeft
        | StdOp::SequencesExtFoldRight
        | StdOp::SequencesExtFoldLeftDomain
        | StdOp::SequencesExtFoldRightDomain => fold_seq(m, op, args),
        StdOp::SequencesExtSelectInSeq | StdOp::SequencesExtSelectLastInSeq => {
            let s = value(args, 0)?;
            let Some(t) = tuple(m, op, s)? else {
                return Err(arg_error("first", op.name(), "sequence"));
            };
            let test = func(args, 1)?;
            let last = op == StdOp::SequencesExtSelectLastInSeq;
            let positions: Vec<usize> = if last {
                (0..t.len()).rev().collect()
            } else {
                (0..t.len()).collect()
            };
            for i in positions {
                if test_bool(m, op, &test, t[i])? {
                    return int(i + 1);
                }
            }
            int(0)
        }
        StdOp::SequencesExtRemoveFirstMatch => {
            let Some(t) = tuple(m, op, value(args, 0)?)? else {
                return Err(arg_error("first", "RemoveFirstMatch", "sequence"));
            };
            let test = func(args, 1)?;
            let mut out = Vec::with_capacity(t.len());
            let mut found = false;
            for y in t {
                if !found && test_bool(m, op, &test, y)? {
                    found = true;
                    continue;
                }
                out.push(y);
            }
            Ok(m.heap.tuple(&out))
        }
        _ => Err(Error::internal("not a higher-order library operator")),
    }
}

fn value(args: &[Ho<'_>], i: usize) -> Result<Handle, Error> {
    match args.get(i) {
        Some(Ho::Value(h)) => Ok(*h),
        _ => Err(Error::internal("a value argument expected")),
    }
}

fn func<'a>(args: &[Ho<'a>], i: usize) -> Result<OpFn<'a>, Error> {
    match args.get(i) {
        Some(Ho::Op(f)) => Ok(*f),
        _ => Err(Error::internal("an operator argument expected")),
    }
}

fn int(n: usize) -> Result<Handle, Error> {
    i64::try_from(n)
        .ok()
        .and_then(Handle::int)
        .ok_or_else(|| Error::internal("an index"))
}

/// The elements of a sequence; `None` for any other value.
fn tuple(m: &Machine, op: StdOp, h: Handle) -> Result<Option<Vec<Handle>>, Error> {
    if !h.is_compound() {
        return Ok(None);
    }
    let v = m.heap.view(h);
    match v.kind {
        Kind::IntFcn if v.lo == 1 || v.data.is_empty() => Ok(Some(v.data.to_vec())),
        Kind::Lambda => Err(refuse(op, "a function argument the core keeps lazy")),
        _ => Ok(None),
    }
}

/// A one-argument test. A non-boolean result is TLC's error naming the
/// operator, which the reference engine refuses; so does the core.
fn test_bool(m: &mut Machine, op: StdOp, test: &OpFn<'_>, x: Handle) -> Result<bool, Error> {
    match test.apply(m, &[x])?.as_bool() {
        Some(b) => Ok(b),
        None => Err(refuse(
            op,
            "a test that is not boolean-valued (TLC's message prints the operator)",
        )),
    }
}

/// `SelectSeq(s, Test)`.
fn select_seq(m: &mut Machine, op: StdOp, s: Handle, test: OpFn<'_>) -> Result<Handle, Error> {
    let Some(t) = tuple(m, op, s)? else {
        return Err(arg_error("first", "SelectSeq", "sequence"));
    };
    let mut out = Vec::new();
    for x in t {
        match test.apply(m, &[x])?.as_bool() {
            Some(true) => out.push(x),
            Some(false) => {}
            // The pinned TLC reports "operator" and the result here.
            None => return Err(arg_error("second", "SelectSeq", "operator")),
        }
    }
    Ok(m.heap.tuple(&out))
}

/// `SortSeq(s, Op)`: TLC's insertion sort (`TLC.SortSeq`).
fn sort_seq(m: &mut Machine, op: StdOp, s: Handle, less: OpFn<'_>) -> Result<Handle, Error> {
    let Some(t) = tuple(m, op, s)? else {
        return Err(arg_error("first", "SortSeq", "natural number"));
    };
    let Some((&first, rest)) = t.split_first() else {
        return Ok(m.heap.tuple(&[]));
    };
    let mut out = vec![first];
    for &x in rest {
        let mut j = out.len();
        out.push(x);
        loop {
            let Some(lt) = less.apply(m, &[x, out[j - 1]])?.as_bool() else {
                // The pinned TLC reports "operator" and the result here.
                return Err(arg_error("second", "SortSeq", "operator"));
            };
            if !lt {
                break;
            }
            out[j] = out[j - 1];
            j -= 1;
            if j == 0 {
                break;
            }
        }
        out[j] = x;
    }
    Ok(m.heap.tuple(&out))
}

/// Whether TLC walks a function's entries in the order the core holds
/// them: a sequence, or at most one entry. Other functions TLC may hold
/// unnormalized (`:>`, `@@`, `Bags`' results).
fn order_known(m: &Machine, f: Handle) -> bool {
    let v = m.heap.view(f);
    v.len() <= 1 || (v.kind == Kind::IntFcn && v.lo == 1)
}

/// `BagOfAll(F, B)`: the bag of `F(e)`, each with `e`'s count. The result
/// does not depend on the order; an error applying `F` does, so over a
/// bag TLC may hold unnormalized it is refused.
fn bag_of_all(m: &mut Machine, op: StdOp, f: OpFn<'_>, b: Handle) -> Result<Handle, Error> {
    let Some((keys, vals)) = bags::entries(m, op, b)? else {
        return Err(arg_error(
            "second",
            "BagOfAll",
            "function with a finite domain",
        ));
    };
    bags::counts(m, op, &vals)?;
    let known = order_known(m, b);
    let mut acc: Vec<(Handle, Handle)> = Vec::new();
    for (d, n) in keys.into_iter().zip(vals) {
        let x = match f.apply(m, &[d]) {
            Ok(x) => x,
            Err(e) if known || e.kind() != crate::ErrorKind::Eval => return Err(e),
            Err(_) => {
                return Err(refuse(
                    op,
                    "an error applying the operator over a bag TLC may hold unnormalized",
                ));
            }
        };
        bags::accumulate(m, op, &mut acc, x, n)?;
    }
    bags::bag(m, &mut acc)
}

/// `FoldBag(op, base, B)`: folds each element as many times as its count,
/// in the bag's order, which is known only for a sequence or a bag of at
/// most one element.
fn fold_bag(
    m: &mut Machine,
    op: StdOp,
    f: OpFn<'_>,
    base: Handle,
    b: Handle,
) -> Result<Handle, Error> {
    let Some((keys, vals)) = bags::entries(m, op, b)? else {
        return Err(arg_error("third", "FoldBag", "bag"));
    };
    if !order_known(m, b) {
        return Err(refuse(
            op,
            "a bag of two or more elements: TLC folds in the order it holds the bag in, which may be unnormalized",
        ));
    }
    let mut acc = base;
    for (d, x) in keys.into_iter().zip(vals) {
        if !bags::positive(m, x) {
            return Err(exc(
                2176,
                "Applying FoldBag to the following value,\nwhich is not an element of Nat:",
            ));
        }
        let Some(n) = x.as_int().filter(|n| i32::try_from(*n).is_ok()) else {
            return Err(refuse(op, "a count outside TLC's 32-bit integers"));
        };
        for _ in 0..n {
            acc = f.apply(m, &[acc, d])?;
        }
    }
    Ok(acc)
}

/// `Quantify(S, P)`: the number of elements of `S` satisfying `P`.
fn quantify(m: &mut Machine, op: StdOp, s: Handle, test: OpFn<'_>) -> Result<Handle, Error> {
    let Some(elems) = enumerable(m, op, s)? else {
        return Err(arg_error("first", "Quantify", "set"));
    };
    let mut n = 0usize;
    for x in elems {
        if test_bool(m, op, &test, x)? {
            n += 1;
        }
    }
    int(n)
}

/// `FoldFunction(op, base, f)` (and `FoldSeq`): folds `op(f[x], acc)`
/// over `f`'s domain in its normalized order.
fn fold_function(
    m: &mut Machine,
    op: StdOp,
    f: OpFn<'_>,
    base: Handle,
    fun: Handle,
) -> Result<Handle, Error> {
    let Some((_, vals)) = bags::entries(m, op, fun)? else {
        return Err(arg_error("third", "FoldFunction", "function"));
    };
    let mut acc = base;
    for y in vals {
        acc = f.apply(m, &[y, acc])?;
    }
    Ok(acc)
}

/// `FoldLeft(op, base, s)`, `FoldRight(op, s, base)` and their `Domain`
/// forms, which fold the indices instead of the elements.
fn fold_seq(m: &mut Machine, op: StdOp, args: &[Ho<'_>]) -> Result<Handle, Error> {
    let left = matches!(
        op,
        StdOp::SequencesExtFoldLeft | StdOp::SequencesExtFoldLeftDomain
    );
    let domain = matches!(
        op,
        StdOp::SequencesExtFoldLeftDomain | StdOp::SequencesExtFoldRightDomain
    );
    let f = func(args, 0)?;
    let (base, s) = if left {
        (value(args, 1)?, value(args, 2)?)
    } else {
        (value(args, 2)?, value(args, 1)?)
    };
    let Some(t) = tuple(m, op, s)? else {
        return Err(arg_error(
            if left { "third" } else { "second" },
            op.name(),
            "sequence",
        ));
    };
    let item = |i: usize| -> Result<Handle, Error> { if domain { int(i + 1) } else { Ok(t[i]) } };
    let mut acc = base;
    if left {
        for i in 0..t.len() {
            acc = f.apply(m, &[acc, item(i)?])?;
        }
    } else {
        for i in (0..t.len()).rev() {
            acc = f.apply(m, &[item(i)?, acc])?;
        }
    }
    Ok(acc)
}
