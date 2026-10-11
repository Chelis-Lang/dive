//! `--comparisons=tla` (the owner-approved default, D-009): `=` and `\in` answered as TLA+ defines them for functions
//! (including tuples and records) and sets, symmetrically and independently of
//! element order.
//!
//! The rules:
//! - Two scalars of the same kind (booleans, integers, strings, model values)
//!   compare by value.
//! - Where TLA+ leaves the answer unspecified, TLC's error is kept: a direct
//!   comparison of scalars of different kinds (`1 = "a"`), or of a set, a
//!   function or a scalar with a value of another of those three kinds
//!   (`{1} = 1`). TLC's own answer is used when TLC gives one (an untyped model
//!   value is unequal to everything else).
//! - Inside sets and functions, values of different kinds are unequal, so
//!   `[a |-> 1, b |-> 1] = <<1>>` and `<<1>> \in [a : {1}]` are `FALSE`.
//! - Set enumerations are still normalized by TLC's rules: this option
//!   changes comparisons, not set construction.

use dive_value::{SetKind, Value, tlc};

use crate::core::Core;
use crate::error::Result;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Class {
    Scalar(u8),
    Set,
    Fcn,
}

fn class(v: &Value) -> Class {
    match v {
        Value::Bool(_) => Class::Scalar(0),
        Value::Int(_) | Value::BigInt(_) => Class::Scalar(1),
        Value::Str(_) => Class::Scalar(2),
        Value::Model(_) => Class::Scalar(3),
        Value::Set(_) => Class::Set,
        Value::Fcn(_) => Class::Fcn,
    }
}

/// `a = b`.
pub(crate) fn equals(core: &Core, a: &Value, b: &Value) -> Result<bool> {
    let (ca, cb) = (class(a), class(b));
    if ca == cb && ca != Class::Scalar(3) {
        // Same kind: TLA+'s extensional equality, which `Value`'s total
        // equality implements (values of different kinds inside are unequal).
        return Ok(a == b);
    }
    // Unspecified in TLA+, or model values: TLC's answer or TLC's error.
    Ok(tlc::equals(a, b, core.ctx)?)
}

/// `elem \in set`.
pub(crate) fn member(core: &Core, elem: &Value, set: &Value) -> Result<bool> {
    let Value::Set(s) = set else {
        return Ok(tlc::member(elem, set, core.ctx)?);
    };
    match s.kind() {
        SetKind::FcnSet | SetKind::RcdSet | SetKind::Product | SetKind::Seq => {
            if class(elem) != Class::Fcn {
                return Ok(tlc::member(elem, set, core.ctx)?);
            }
            Ok(structural_member(core, elem, set)?)
        }
        SetKind::Subset => {
            if class(elem) != Class::Set {
                return Ok(tlc::member(elem, set, core.ctx)?);
            }
            let comps = s.components();
            let Some((_, base)) = comps.first() else {
                return Ok(false);
            };
            let Value::Set(es) = elem else {
                return Ok(false);
            };
            for x in es.elements(core.ctx)?.iter() {
                if !member(core, x, base)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        SetKind::Enum | SetKind::Raw => {
            // An enumeration TLC has not normalized is scanned as written, so
            // that the answer does not depend on normalizing it.
            let elems: std::sync::Arc<[Value]> = match s.raw_elements() {
                Some(raw) => raw.into(),
                None => s.elements(core.ctx)?,
            };
            let ce = class(elem);
            let mut unspecified = false;
            for x in elems.iter() {
                let cx = class(x);
                if ce == cx && ce != Class::Scalar(3) {
                    if elem == x {
                        return Ok(true);
                    }
                } else if matches!(ce, Class::Scalar(_))
                    || matches!(cx, Class::Scalar(_))
                    || ce == Class::Scalar(3)
                    || cx == Class::Scalar(3)
                {
                    // A comparison TLA+ leaves unspecified (or a model value):
                    // TLC's answer decides it, or the membership is unspecified.
                    match tlc::equals(elem, x, core.ctx) {
                        Ok(true) => return Ok(true),
                        Ok(false) => {}
                        Err(_) => unspecified = true,
                    }
                }
            }
            if unspecified {
                return Ok(tlc::member(elem, set, core.ctx)?);
            }
            Ok(false)
        }
        _ => Ok(tlc::member(elem, set, core.ctx)?),
    }
}

/// Membership of a function in `[S -> T]`, `[a : S, ...]`, `S1 \X ... \X Sn`
/// or `Seq(S)`: the domain must be the set's, and each value a member.
fn structural_member(core: &Core, elem: &Value, set: &Value) -> Result<bool> {
    let (Value::Fcn(f), Value::Set(s)) = (elem, set) else {
        return Ok(false);
    };
    let comps = s.components();
    match s.kind() {
        SetKind::RcdSet => {
            let Some(fields) = f.as_record() else {
                return Ok(false);
            };
            if fields.len() != comps.len() {
                return Ok(false);
            }
            for (name, sub) in &comps {
                let Some(name) = name else {
                    return Ok(false);
                };
                let Some((_, v)) = fields.iter().find(|(a, _)| a == name) else {
                    return Ok(false);
                };
                if !member(core, v, sub)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        SetKind::Product => {
            let Some(t) = f.as_tuple() else {
                return Ok(false);
            };
            if t.len() != comps.len() {
                return Ok(false);
            }
            for (x, (_, sub)) in t.iter().zip(&comps) {
                if !member(core, x, sub)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        SetKind::Seq => {
            let Some(t) = f.as_tuple() else {
                return Ok(false);
            };
            let Some((_, range)) = comps.first() else {
                return Ok(false);
            };
            for x in t {
                if !member(core, x, range)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        SetKind::FcnSet => {
            let (Some((_, dom)), Some((_, range))) = (comps.first(), comps.get(1)) else {
                return Ok(false);
            };
            let fdom = Value::set(f.iter().map(|(d, _)| d).collect(), core.ctx)?;
            if !equals(core, &fdom, dom)? {
                return Ok(false);
            }
            for (_, v) in f.iter() {
                if !member(core, v, range)? {
                    return Ok(false);
                }
            }
            Ok(true)
        }
        _ => Ok(false),
    }
}
