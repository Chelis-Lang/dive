//! The value operators of the `TLC` module (`tlc2.module.TLC`): `:>`, `@@`
//! `Assert`, `Permutations` and `ToString` (printed by `crate::print`), and
//! the effects `Print` and `PrintT`, whose output the machine keeps in
//! order (`Machine::printed`), and `RandomElement`, which draws from the
//! machine's stream (`crate::random`), and `TLCGet`/`TLCSet`, against the
//! run's registers. The reference is
//! `crates/dive-eval/src/std.rs` (`TlcMakeFcn`, `combine_fcn`, `TlcAssert`,
//! `permutations`).

use dive_ir::StdOp;

use super::{arg, arg_error, enumerable, exc, refuse, wrong_value};
use crate::compare;
use crate::eval::Machine;
use crate::node::Kind;
use crate::strings::StringOrder;
use crate::{Error, Handle};

pub(super) fn implements(op: StdOp) -> bool {
    matches!(
        op,
        StdOp::TlcMakeFcn
            | StdOp::TlcCombineFcn
            | StdOp::TlcAssert
            | StdOp::TlcPermutations
            | StdOp::TlcToString
            | StdOp::TlcPrint
            | StdOp::TlcPrintT
            | StdOp::TlcRandomElement
            | StdOp::TlcTLCGet
            | StdOp::TlcTLCSet
    )
}

pub(super) fn call(m: &mut Machine, op: StdOp, args: &[Handle]) -> Result<Handle, Error> {
    match op {
        StdOp::TlcMakeFcn => {
            let (d, e) = (arg(args, 0)?, arg(args, 1)?);
            Ok(m.heap.fcn_sorted(&[d], &[e]))
        }
        StdOp::TlcCombineFcn => combine(m, op, arg(args, 0)?, arg(args, 1)?),
        StdOp::TlcAssert => {
            let a = arg(args, 0)?;
            if a.as_bool() == Some(true) {
                return Ok(a);
            }
            Err(exc(
                2132,
                "The first argument of Assert evaluated to FALSE; the second argument was:",
            ))
        }
        StdOp::TlcPermutations => permutations(m, op, arg(args, 0)?),
        StdOp::TlcPrint => {
            let (a, b) = (arg(args, 0)?, arg(args, 1)?);
            let text = format!(
                "{}  {}",
                crate::print::pretty(&mut m.heap, a)?,
                crate::print::pretty(&mut m.heap, b)?
            );
            m.printed.push(text);
            Ok(b)
        }
        StdOp::TlcPrintT => {
            let text = crate::print::pretty(&mut m.heap, arg(args, 0)?)?;
            m.printed.push(text);
            Ok(Handle::TRUE)
        }
        StdOp::TlcRandomElement => random_element(m, op, arg(args, 0)?),
        StdOp::TlcTLCGet | StdOp::TlcTLCSet => register(m, op, args),
        StdOp::TlcToString => {
            let text = crate::print::tlc_string(&mut m.heap, arg(args, 0)?)?;
            m.heap.string(&text)
        }
        _ => Err(Error::internal("not a TLC operator")),
    }
}

/// `TLCGet(key)` or `TLCSet(key, value)`: `TLCGet("level")` from the
/// machine, any other key against the run's registers
/// (`03-value-model.md` §6.2), on the arguments converted to reference
/// values and the result converted back.
fn register(m: &mut Machine, op: StdOp, args: &[Handle]) -> Result<Handle, Error> {
    let Some(p) = m.heap.store().printer().cloned() else {
        return Err(refuse(op, "registers without the model's names (M1 core)"));
    };
    let mut vals = Vec::with_capacity(args.len());
    for &h in args {
        vals.push(
            crate::print::to_value(
                &mut m.heap,
                &p.names,
                h,
                p.ctx,
                crate::print::Conversion::Report,
            )
            .map_err(Error::unsupported)?,
        );
    }
    if op == StdOp::TlcTLCGet
        && matches!(vals.first(), Some(dive_value::Value::Str(k)) if k.as_str() == "level")
    {
        return Handle::int(m.level)
            .ok_or_else(|| Error::unsupported("a level beyond the core's integers (M1 core)"));
    }
    let Some(registers) = m.heap.store().registers().cloned() else {
        return Err(refuse(
            op,
            "registers outside a model-checking run (M1 core)",
        ));
    };
    let out = registers.call(op, &vals)?;
    crate::convert::from_value(&mut m.heap, &out, p.ctx)
}

/// `RandomElement(S)`, drawn by the reference engine's operator from the
/// machine's stream, on `S` converted with its lazy form
/// (`03-value-model.md` §6.1).
fn random_element(m: &mut Machine, op: StdOp, s: Handle) -> Result<Handle, Error> {
    let s = if s.is_compound() && m.heap.view(s).is_lazy() {
        m.work_norm(s)?
    } else {
        s
    };
    if let Some(why) = draw_ambiguous(m, s) {
        return Err(refuse(op, why));
    }
    let Some(p) = m.heap.store().printer().cloned() else {
        return Err(refuse(op, "a draw without the model's names (M1 core)"));
    };
    let v = crate::print::to_value(
        &mut m.heap,
        &p.names,
        s,
        p.ctx,
        crate::print::Conversion::Draw,
    )
    .map_err(Error::unsupported)?;
    let Some(stream) = m.draw.as_mut() else {
        return Err(refuse(
            op,
            "a draw outside the search's random streams (M1 core)",
        ));
    };
    m.draws += 1;
    let out = stream.draw(op, &[v], p.ctx)?;
    crate::convert::from_value(&mut m.heap, &out, p.ctx)
}

/// Why TLC might draw otherwise from `s`: one value per component from a
/// lazy `[S -> T]`, `[a : S]` or `S \X T`, which the core may hold
/// enumerated when the plan builds such sets.
fn draw_ambiguous(m: &Machine, s: Handle) -> Option<&'static str> {
    if !s.is_compound() || !m.heap.store().lazy_sets() {
        return None;
    }
    let v = m.heap.view(s);
    match v.kind {
        Kind::Set => v
            .values()
            .iter()
            .any(|&e| {
                e.is_compound() && matches!(m.heap.view(e).kind, Kind::IntFcn | Kind::Fcn)
            })
            .then_some(
                "an enumerated set of functions, which TLC may hold as a lazy function, record or product set and draw from per component (M1 core)",
            ),
        Kind::FcnSet | Kind::RecSet | Kind::Product | Kind::Subset => {
            v.data.iter().find_map(|&d| draw_ambiguous(m, d))
        }
        _ => None,
    }
}

/// A function's entries, `None` for a value that is not a function.
fn pairs(m: &Machine, op: StdOp, h: Handle) -> Result<Option<Vec<(Handle, Handle)>>, Error> {
    if !h.is_compound() {
        return Ok(None);
    }
    let v = m.heap.view(h);
    match v.kind {
        Kind::IntFcn => {
            let mut out = Vec::with_capacity(v.data.len());
            for (k, &x) in (v.lo..).zip(v.data) {
                let k = Handle::int(k).ok_or_else(|| Error::internal("function domain"))?;
                out.push((k, x));
            }
            Ok(Some(out))
        }
        Kind::Fcn => Ok(Some(
            v.keys()
                .iter()
                .copied()
                .zip(v.values().iter().copied())
                .collect(),
        )),
        Kind::Lambda => Err(refuse(op, "a function argument the core keeps lazy")),
        _ => Ok(None),
    }
}

/// `f @@ g`: `f`'s entries, then those of `g` whose argument is not in
/// `f`'s domain, compared with `equals` (TLC's `TLC.CombineFcn`).
fn combine(m: &mut Machine, op: StdOp, f: Handle, g: Handle) -> Result<Handle, Error> {
    let Some(mut out) = pairs(m, op, f)? else {
        return Err(arg_error("first", "@@", "function"));
    };
    let Some(second) = pairs(m, op, g)? else {
        return Err(arg_error("second", "@@", "function"));
    };
    let n1 = out.len();
    for (d, x) in second {
        let mut found = false;
        for &(d1, _) in &out[..n1] {
            if compare::equals(&m.heap, d, d1)? {
                found = true;
                break;
            }
        }
        if !found {
            out.push((d, x));
        }
    }
    out.sort_unstable_by(|a, b| m.heap.cmp(a.0, b.0));
    let keys: Vec<Handle> = out.iter().map(|p| p.0).collect();
    if compare::check_set(&m.heap, &keys).is_err() {
        return Err(refuse(
            op,
            "a domain mixing values TLC cannot compare (TLC builds it unnormalized and fails only later)",
        ));
    }
    let vals: Vec<Handle> = out.iter().map(|p| p.1).collect();
    Ok(m.heap.fcn_sorted(&keys, &vals))
}

/// `Permutations(S)`: the bijections of `S`. TLC builds them in the
/// lexicographic order of their values over `S` in TLC's order, as an
/// unnormalized set. That is the normalized order of the result when the
/// core orders `S`'s elements as TLC does: integers and booleans always,
/// strings and model values under the token order. Other domains are
/// refused.
fn permutations(m: &mut Machine, op: StdOp, s: Handle) -> Result<Handle, Error> {
    let Some(domain) = enumerable(m, op, s)? else {
        return Err(wrong_value("Permutations", "a finite set"));
    };
    let token = m.heap.store().strings().order() == StringOrder::Token;
    let ordered = domain.iter().all(|&e| {
        e.as_int().is_some()
            || e.as_bool().is_some()
            || (token && (e.as_str_id().is_some() || e.as_model_id().is_some()))
    });
    if !ordered {
        return Err(refuse(
            op,
            "a set whose order in the core may differ from TLC's, which orders the permutations",
        ));
    }
    let n = domain.len();
    let mut out = Vec::new();
    let mut idx: Vec<usize> = (0..n).collect();
    loop {
        let vals: Vec<Handle> = idx.iter().map(|&i| domain[i]).collect();
        out.push(m.heap.fcn_sorted(&domain, &vals));
        if !next_permutation(&mut idx) {
            break;
        }
    }
    Ok(m.heap.set_from(&mut out))
}

/// Advances `idx` to its next permutation in lexicographic order; false
/// after the last.
pub(super) fn next_permutation(idx: &mut [usize]) -> bool {
    let Some(i) = idx.windows(2).rposition(|w| w[0] < w[1]) else {
        return false;
    };
    let Some(j) = idx.iter().rposition(|&x| x > idx[i]) else {
        return false;
    };
    idx.swap(i, j);
    idx[i + 1..].reverse();
    true
}
