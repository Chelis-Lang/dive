//! CommunityModules operators that take only values: `SequencesExt`
//! (`Contains`, `RemoveFirst`, `Suffixes`, `SetToSeq`, `IsPrefix`),
//! `Functions` (`IsInjective`) and `Bitwise`. The reference is
//! `crates/dive-eval/src/community.rs` (`call_ext`), which follows the Java
//! overrides of `tlc2.overrides` (`docs/provenance.md`).

use dive_ir::StdOp;

use super::{arg, arg_error, enumerable, exc, refuse};
use crate::compare;
use crate::eval::Machine;
use crate::node::Kind;
use crate::{Error, Handle};

pub(super) fn implements(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::SequencesExtContains
            | StdOp::SequencesExtRemoveFirst
            | StdOp::SequencesExtSuffixes
            | StdOp::SequencesExtSetToSeq
            | StdOp::SequencesExtSetToSeqs
            | StdOp::SequencesExtIsPrefix
            | StdOp::FunctionsIsInjective
            | StdOp::FunctionsAntiFunction
            | StdOp::BitwiseAnd
            | StdOp::BitwiseOr
            | StdOp::BitwiseXor
            | StdOp::BitwiseNot
            | StdOp::BitwiseShiftR
    )
}

pub(super) fn call(m: &mut Machine, op: StdOp, args: &[Handle]) -> Result<Handle, Error> {
    match op {
        StdOp::SequencesExtContains => {
            let Some(t) = tuple(m, op, arg(args, 0)?)? else {
                return Err(one_arg_error("Contains", "sequence"));
            };
            let x = arg(args, 1)?;
            for y in t {
                if compare::equals(&m.heap, y, x)? {
                    return Ok(Handle::TRUE);
                }
            }
            Ok(Handle::FALSE)
        }
        StdOp::SequencesExtRemoveFirst => {
            let Some(t) = tuple(m, op, arg(args, 0)?)? else {
                return Err(arg_error("first", "RemoveFirst", "sequence"));
            };
            let x = arg(args, 1)?;
            let mut out = Vec::with_capacity(t.len());
            let mut found = false;
            for y in t {
                if !found && compare::equals(&m.heap, y, x)? {
                    found = true;
                } else {
                    out.push(y);
                }
            }
            Ok(m.heap.tuple(&out))
        }
        StdOp::SequencesExtSuffixes => {
            let Some(t) = tuple(m, op, arg(args, 0)?)? else {
                return Err(one_arg_error("Suffixes", "sequence"));
            };
            let mut out = Vec::with_capacity(t.len() + 1);
            for i in 0..=t.len() {
                out.push(m.heap.tuple(&t[i..]));
            }
            Ok(m.heap.set_from(&mut out))
        }
        StdOp::SequencesExtSetToSeq => {
            let Some(elems) = enumerable(m, op, arg(args, 0)?)? else {
                return Err(refuse(
                    op,
                    "a value that is not an enumerable set (TLC fails with a NullPointerException)",
                ));
            };
            Ok(m.heap.tuple(&elems))
        }
        StdOp::SequencesExtSetToSeqs => set_to_seqs(m, op, arg(args, 0)?),
        StdOp::SequencesExtIsPrefix => is_prefix(m, op, arg(args, 0)?, arg(args, 1)?),
        StdOp::FunctionsIsInjective => is_injective(m, op, arg(args, 0)?),
        StdOp::FunctionsAntiFunction => anti_function(m, op, arg(args, 0)?),
        StdOp::BitwiseAnd | StdOp::BitwiseOr | StdOp::BitwiseXor => {
            let x = java_int(op, arg(args, 0)?)?;
            let y = java_int(op, arg(args, 1)?)?;
            java_int(op, arg(args, 2)?)?;
            java_int(op, arg(args, 3)?)?;
            let r = match op {
                StdOp::BitwiseAnd => x & y,
                StdOp::BitwiseOr => x | y,
                _ => x ^ y,
            };
            int(i64::from(r))
        }
        StdOp::BitwiseNot => {
            let x = java_int(op, arg(args, 0)?)?;
            // `Integer.MAX_VALUE >> Integer.numberOfLeadingZeros(x) - 1`:
            // Java masks the shift distance to five bits.
            let shift = x.leading_zeros().wrapping_sub(1) & 31;
            let mask = i32::MAX >> shift;
            int(i64::from(!x & mask))
        }
        StdOp::BitwiseShiftR => {
            let n = java_int(op, arg(args, 0)?)?;
            let pos = java_int(op, arg(args, 1)?)?;
            // `n >>> pos`.
            let r = (n.cast_unsigned() >> (pos.cast_unsigned() & 31)).cast_signed();
            int(i64::from(r))
        }
        _ => Err(Error::internal("not a CommunityModules operator")),
    }
}

/// `SetToSeqs(S)`: every ordering of `S` as a sequence. TLC builds them
/// unnormalized, in the lexicographic order of the index permutations
/// over `S`'s normalized elements; for tuples of one length that is the
/// normalized order, in either profile, so the core returns the set.
fn set_to_seqs(m: &mut Machine, op: StdOp, s: Handle) -> Result<Handle, Error> {
    let Some(elems) = enumerable(m, op, s)? else {
        return Err(super::wrong_value("SetToSeqs", "a finite set"));
    };
    if elems.len() > 12 {
        // TLC's `int factorial` overflows from 13 elements on.
        return Err(refuse(op, "a set of more than 12 elements"));
    }
    let mut idx: Vec<usize> = (0..elems.len()).collect();
    let mut out = Vec::new();
    loop {
        let vals: Vec<Handle> = idx.iter().map(|&i| elems[i]).collect();
        out.push(m.heap.tuple(&vals));
        if !super::tlc::next_permutation(&mut idx) {
            break;
        }
    }
    Ok(m.heap.set_from(&mut out))
}

/// `EC.TLC_MODULE_ONE_ARGUMENT_ERROR` (2283).
fn one_arg_error(op: &str, what: &str) -> Error {
    exc(
        2283,
        format!("The argument of {op} should be a {what}, but instead it is:"),
    )
}

fn int(n: i64) -> Result<Handle, Error> {
    Handle::int(n).ok_or_else(|| Error::internal("a 32-bit integer"))
}

/// A Java `int` argument; any other value is refused, as the reference
/// engine refuses it (TLC fails with a cast or range error).
fn java_int(op: StdOp, h: Handle) -> Result<i32, Error> {
    match h.as_int() {
        Some(i) => {
            i32::try_from(i).map_err(|_| refuse(op, "an integer outside TLC's 32-bit range"))
        }
        None => Err(refuse(op, "an argument that is not a TLC IntValue")),
    }
}

/// The elements of a sequence (a function over `1..n`); `None` for any
/// other value.
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

/// `IsPrefix(s, t)`: string prefix on two strings, else elementwise on
/// sequences.
fn is_prefix(m: &mut Machine, op: StdOp, lhs: Handle, rhs: Handle) -> Result<Handle, Error> {
    if let (Some(p), Some(q)) = (lhs.as_str_id(), rhs.as_str_id()) {
        let strings = m.heap.store().strings();
        let (Some(prefix), Some(whole)) = (strings.text(p), strings.text(q)) else {
            return Err(Error::internal("unknown string"));
        };
        return Ok(Handle::bool(whole.starts_with(&*prefix)));
    }
    let (Some(short), Some(long)) = (tuple(m, op, lhs)?, tuple(m, op, rhs)?) else {
        return Err(refuse(
            op,
            "an argument that is not a sequence (TLC fails with a NullPointerException)",
        ));
    };
    if short.len() > long.len() {
        return Ok(Handle::FALSE);
    }
    for (&x, &y) in short.iter().zip(&long) {
        if !compare::equals(&m.heap, x, y)? {
            return Ok(Handle::FALSE);
        }
    }
    Ok(Handle::TRUE)
}

/// `IsInjective(f)`: no two values of `f` are equal. A comparison TLC
/// cannot make is refused: which fails first depends on how TLC holds the
/// function.
fn is_injective(m: &mut Machine, op: StdOp, f: Handle) -> Result<Handle, Error> {
    let values = if f.is_compound() {
        let v = m.heap.view(f);
        match v.kind {
            Kind::IntFcn => Some(v.data.to_vec()),
            Kind::Fcn => Some(v.values().to_vec()),
            Kind::Lambda => return Err(refuse(op, "a function argument the core keeps lazy")),
            Kind::RecSet => {
                return Err(refuse(op, "a set of records"));
            }
            _ => None,
        }
    } else {
        None
    };
    let Some(values) = values else {
        return Err(one_arg_error("IsInjective", "function"));
    };
    for (i, &x) in values.iter().enumerate() {
        for &y in &values[i + 1..] {
            match compare::equals(&m.heap, x, y) {
                Ok(true) => return Ok(Handle::FALSE),
                Ok(false) => {}
                Err(_) => {
                    return Err(refuse(
                        op,
                        "values TLC cannot compare (which comparison fails first depends on how TLC holds the function)",
                    ));
                }
            }
        }
    }
    Ok(Handle::TRUE)
}

/// `AntiFunction(f)`: `[y \in Range(f) |-> x]`, keeping for each value the
/// first argument in the order of a stable sort of `f`'s entries by value
/// (TLC's `Arrays.sort`). Values TLC cannot compare are refused, as the
/// reference engine refuses them.
fn anti_function(m: &mut Machine, op: StdOp, f: Handle) -> Result<Handle, Error> {
    let pairs: Vec<(Handle, Handle)> = if f.is_compound() {
        let v = m.heap.view(f);
        match v.kind {
            Kind::IntFcn => {
                let mut out = Vec::with_capacity(v.data.len());
                for (k, &x) in (v.lo..).zip(v.data) {
                    out.push((int(k)?, x));
                }
                out
            }
            Kind::Fcn => v
                .keys()
                .iter()
                .copied()
                .zip(v.values().iter().copied())
                .collect(),
            Kind::Lambda => return Err(refuse(op, "a function argument the core keeps lazy")),
            _ => return Err(one_arg_error("AntiFunction", "functions")),
        }
    } else {
        return Err(one_arg_error("AntiFunction", "functions"));
    };
    let mut order: Vec<usize> = (0..pairs.len()).collect();
    order.sort_by(|&a, &b| m.heap.cmp(pairs[a].1, pairs[b].1));
    let sorted: Vec<Handle> = order.iter().map(|&i| pairs[i].1).collect();
    if compare::check_set(&m.heap, &sorted).is_err() {
        return Err(refuse(
            op,
            "values TLC cannot compare (TLC's error depends on its sort's order of comparisons)",
        ));
    }
    let mut out: Vec<(Handle, Handle)> = Vec::new();
    for i in order {
        let (arg, value) = pairs[i];
        let dup = match out.last() {
            Some(&(prev, _)) => compare::equals(&m.heap, value, prev)?,
            None => false,
        };
        if !dup {
            out.push((value, arg));
        }
    }
    Ok(m.heap.fcn_from_pairs(&mut out))
}
