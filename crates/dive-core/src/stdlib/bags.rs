//! `Bags` (`tlc2.module.Bags`): the operators beyond the five the evaluator
//! builds itself (`EmptyBag`, `SetToBag`, `BagToSet`, `BagIn`, `CopiesIn`).
//! The reference is `crates/dive-eval/src/community.rs` (`bag_cardinality`,
//! `bag_cup`, `bag_diff`, `bag_union`, `sq_subseteq`).
//!
//! A bag is a function from elements to positive counts. TLC walks a bag's
//! entries comparing elements with `equals`; the core walks them in the
//! function's canonical order, as the reference engine does, and raises the
//! comparison errors it raises.

use std::cmp::Ordering;

use dive_ir::StdOp;

use super::{arg, arg_error, exc, refuse, wrong_value};
use crate::compare;
use crate::eval::Machine;
use crate::int::{IntMode, IntOp};
use crate::node::Kind;
use crate::{Error, Handle};

pub(super) fn implements(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::BagsIsABag
            | StdOp::BagsBagCardinality
            | StdOp::BagsBagCup
            | StdOp::BagsBagDiff
            | StdOp::BagsBagUnion
            | StdOp::BagsSqSubseteq
    )
}

pub(super) fn call(m: &mut Machine, op: StdOp, args: &[Handle]) -> Result<Handle, Error> {
    match op {
        StdOp::BagsIsABag => {
            let Some((_, vals)) = entries(m, op, arg(args, 0)?)? else {
                return Ok(Handle::FALSE);
            };
            Ok(Handle::bool(vals.iter().all(|&x| positive(m, x))))
        }
        StdOp::BagsBagCardinality => cardinality(m, op, arg(args, 0)?),
        StdOp::BagsBagCup => cup(m, op, arg(args, 0)?, arg(args, 1)?),
        StdOp::BagsBagDiff => diff(m, op, arg(args, 0)?, arg(args, 1)?),
        StdOp::BagsBagUnion => union(m, op, arg(args, 0)?),
        StdOp::BagsSqSubseteq => sq_subseteq(m, op, arg(args, 0)?, arg(args, 1)?),
        _ => Err(Error::internal("not a Bags operator")),
    }
}

/// A function's keys and values in canonical order; `None` for a value that
/// is not a function (TLC's `toFcnRcd` returns `null`).
type Entries = (Vec<Handle>, Vec<Handle>);

fn entries(m: &Machine, op: StdOp, h: Handle) -> Result<Option<Entries>, Error> {
    if !h.is_compound() {
        return Ok(None);
    }
    let v = m.heap.view(h);
    match v.kind {
        Kind::IntFcn => {
            let mut keys = Vec::with_capacity(v.data.len());
            for i in 0..v.data.len() {
                let k = i64::try_from(i)
                    .ok()
                    .and_then(|i| v.lo.checked_add(i))
                    .and_then(Handle::int)
                    .ok_or_else(|| Error::internal("function domain out of range"))?;
                keys.push(k);
            }
            Ok(Some((keys, v.data.to_vec())))
        }
        Kind::Fcn => Ok(Some((v.keys().to_vec(), v.values().to_vec()))),
        Kind::Lambda => Err(refuse(op, "a function argument the core keeps lazy")),
        _ => Ok(None),
    }
}

/// A count TLC accepts: an integer greater than zero.
fn positive(m: &Machine, x: Handle) -> bool {
    match x.as_int() {
        Some(n) => n > 0,
        None => sign(m, x) == Some(Ordering::Greater),
    }
}

/// The sign of an integer of any size; `None` for a non-integer.
fn sign(m: &Machine, x: Handle) -> Option<Ordering> {
    let zero = Handle::int(0)?;
    m.heap.int_cmp(x, zero)
}

/// The counts of a bag, which TLC casts to `IntValue` without checking.
fn counts(m: &Machine, op: StdOp, vals: &[Handle]) -> Result<(), Error> {
    if vals.iter().all(|&x| m.heap.is_int(x)) {
        Ok(())
    } else {
        Err(refuse(
            op,
            "a bag with a count that is not an integer (TLC fails with a ClassCastException)",
        ))
    }
}

/// `a + b` or `a - b` on counts. TLC adds Java `int`s and wraps on
/// overflow, which the reference engine refuses; so does the core.
fn count_op(
    m: &mut Machine,
    op: StdOp,
    which: IntOp,
    lhs: Handle,
    rhs: Handle,
) -> Result<Handle, Error> {
    if m.heap.int_mode() == IntMode::Tlc {
        let result = match (lhs.as_int(), rhs.as_int(), which) {
            (Some(x), Some(y), IntOp::Add) => x.checked_add(y),
            (Some(x), Some(y), IntOp::Sub) => x.checked_sub(y),
            _ => None,
        };
        return match result.filter(|v| i32::try_from(*v).is_ok()) {
            Some(v) => Handle::int(v).ok_or_else(|| Error::internal("count")),
            None => Err(refuse(op, "an overflow of a count")),
        };
    }
    m.heap.int_arith(which, lhs, rhs)
}

fn cardinality(m: &mut Machine, op: StdOp, b: Handle) -> Result<Handle, Error> {
    let Some((_, vals)) = entries(m, op, b)? else {
        return Err(wrong_value(
            "BagCardinality",
            "a function with a finite domain",
        ));
    };
    let mut n = Handle::int(0).ok_or_else(|| Error::internal("zero"))?;
    for &x in &vals {
        if !positive(m, x) {
            return Err(wrong_value("BagCardinality", "a bag"));
        }
        n = count_op(m, op, IntOp::Add, n, x)?;
    }
    Ok(n)
}

/// Adds `(d, n)` into `acc`, comparing `d` with each element in turn.
fn accumulate(
    m: &mut Machine,
    op: StdOp,
    acc: &mut Vec<(Handle, Handle)>,
    d: Handle,
    n: Handle,
) -> Result<(), Error> {
    for entry in acc.iter_mut() {
        if compare::equals(&m.heap, d, entry.0)? {
            entry.1 = count_op(m, op, IntOp::Add, entry.1, n)?;
            return Ok(());
        }
    }
    acc.push((d, n));
    Ok(())
}

fn cup(m: &mut Machine, op: StdOp, b1: Handle, b2: Handle) -> Result<Handle, Error> {
    let (Some((k1, v1)), Some((k2, v2))) = (entries(m, op, b1)?, entries(m, op, b2)?) else {
        return Err(refuse(
            op,
            "a non-function argument (TLC fails with a NullPointerException)",
        ));
    };
    if !v1.iter().all(|&x| positive(m, x)) {
        return Err(arg_error("first", "(+)", "bag"));
    }
    if !v2.iter().all(|&x| positive(m, x)) {
        return Err(arg_error("second", "(+)", "bag"));
    }
    counts(m, op, &v1)?;
    counts(m, op, &v2)?;
    let mut acc: Vec<(Handle, Handle)> = k1.into_iter().zip(v1).collect();
    for (d, n) in k2.into_iter().zip(v2) {
        accumulate(m, op, &mut acc, d, n)?;
    }
    Ok(m.heap.fcn_from_pairs(&mut acc))
}

fn diff(m: &mut Machine, op: StdOp, b1: Handle, b2: Handle) -> Result<Handle, Error> {
    let Some((k1, v1)) = entries(m, op, b1)? else {
        return Err(arg_error("first", "(-)", "bag"));
    };
    let Some((k2, v2)) = entries(m, op, b2)? else {
        return Err(arg_error("second", "(-)", "bag"));
    };
    counts(m, op, &v1)?;
    counts(m, op, &v2)?;
    let mut out = Vec::new();
    for (d, n1) in k1.into_iter().zip(v1) {
        let mut n = n1;
        for (&d2, &n2) in k2.iter().zip(&v2) {
            if compare::equals(&m.heap, d, d2)? {
                n = count_op(m, op, IntOp::Sub, n, n2)?;
                break;
            }
        }
        if positive(m, n) {
            out.push((d, n));
        }
    }
    Ok(m.heap.fcn_from_pairs(&mut out))
}

fn union(m: &mut Machine, op: StdOp, s: Handle) -> Result<Handle, Error> {
    let enumerable = s.is_compound()
        && !matches!(
            m.heap.view(s).kind,
            Kind::IntFcn
                | Kind::Fcn
                | Kind::Lambda
                | Kind::BigInt
                | Kind::Nat
                | Kind::Int
                | Kind::SeqSet
        );
    if !enumerable || !m.heap.is_finite(s) {
        return Err(wrong_value("BagUnion", "a finite enumerable set"));
    }
    if m.heap.is_lazy(s) {
        return Err(refuse(op, "a set the core keeps lazy"));
    }
    let elems = m.heap.elements(s)?;
    match elems.len() {
        0 => return Ok(m.heap.fcn_sorted(&[], &[])),
        1 => return Ok(elems[0]),
        _ => {}
    }
    let mut acc: Vec<(Handle, Handle)> = Vec::new();
    for &b in &elems {
        let Some((keys, vals)) = entries(m, op, b)? else {
            return Err(exc(
                2177,
                "Attempted to apply BagUnion to the following set, whose\nelement is not a bag:",
            ));
        };
        counts(m, op, &vals)?;
        for (d, n) in keys.into_iter().zip(vals) {
            accumulate(m, op, &mut acc, d, n)?;
        }
    }
    Ok(m.heap.fcn_from_pairs(&mut acc))
}

fn sq_subseteq(m: &mut Machine, op: StdOp, b1: Handle, b2: Handle) -> Result<Handle, Error> {
    let Some((k1, v1)) = entries(m, op, b1)? else {
        return Err(wrong_value(
            "\\sqsubseteq",
            "a function with a finite domain",
        ));
    };
    let Some((k2, v2)) = entries(m, op, b2)? else {
        return Err(wrong_value(
            "\\sqsubseteq",
            "a function with a finite domain",
        ));
    };
    counts(m, op, &v1)?;
    counts(m, op, &v2)?;
    for (d, n1) in k1.into_iter().zip(v1) {
        let mut n = n1;
        for (&d2, &n2) in k2.iter().zip(&v2) {
            if compare::equals(&m.heap, d, d2)? {
                n = count_op(m, op, IntOp::Sub, n, n2)?;
                break;
            }
        }
        if positive(m, n) {
            return Ok(Handle::FALSE);
        }
    }
    Ok(Handle::TRUE)
}
