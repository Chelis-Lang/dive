//! Set sizes and enumeration.
//!
//! [`size`] follows TLC's `size()` methods, including its 32-bit overflow errors in
//! [`IntMode::Tlc`]; [`quiet_size`] is the mathematical size, used by equality,
//! ordering and hashing. Enumeration materializes a [stream](crate::stream) of the
//! elements in normalized order.

use crate::ctx::{Ctx, IntMode};
use crate::error::{ErrorKind, Result, ValueError};
use crate::print::{ppr, to_tlc_string_lenient};
use crate::stream::{not_enumerable, stream};
use crate::tlc::is_empty;
use crate::value::{Set, SetRepr, Value};

/// The most elements dive will materialize in one set.
pub const MAX_ENUMERATION: u64 = i32::MAX as u64;

fn p(v: &Value, ctx: Ctx) -> String {
    ppr(&to_tlc_string_lenient(v, ctx))
}

fn too_large(what: &Value, n: Option<u128>, ctx: Ctx) -> ValueError {
    ValueError::new(
        ErrorKind::TooLarge,
        match n {
            Some(n) => format!(
                "The set\n{}\nhas {n} elements, more than dive enumerates ({MAX_ENUMERATION}).",
                p(what, ctx)
            ),
            None => format!(
                "The set\n{}\nhas more elements than dive enumerates ({MAX_ENUMERATION}).",
                p(what, ctx)
            ),
        },
    )
}

/// Whether a set is infinite. Conservative only for sets built from infinite parts.
pub(crate) fn is_infinite(s: &Set) -> bool {
    let inf = |v: &Value| matches!(v, Value::Set(t) if is_infinite(t));
    match s.repr() {
        SetRepr::Nat | SetRepr::Int | SetRepr::Strings => true,
        SetRepr::Seq(r) => quiet_component_size(r) != Some(0),
        SetRepr::Subset(b) => inf(b),
        SetRepr::FcnSet(d, r) => inf(d) || inf(r),
        SetRepr::RcdSet(fs) => fs.iter().any(|(_, v)| inf(v)),
        SetRepr::Product(fs) => fs.iter().any(inf),
        _ => false,
    }
}

fn quiet_component_size(v: &Value) -> Option<u128> {
    match v {
        Value::Set(s) => quiet_size(s),
        _ => None,
    }
}

/// The mathematical number of elements, or `None` when the set is infinite, cannot
/// be enumerated, or has more than `u128::MAX` elements. Changes nothing observable.
pub(crate) fn quiet_size(s: &Set) -> Option<u128> {
    let quiet = Ctx {
        int_mode: IntMode::Big,
        order: s.order(),
    };
    match s.repr() {
        SetRepr::Enum | SetRepr::Raw(_) | SetRepr::Deferred(_) => {
            s.quiet_elements(quiet).ok().map(|e| e.len() as u128)
        }
        SetRepr::Interval(lo, hi) => {
            if hi < lo {
                return Some(0);
            }
            u128::try_from(hi.to_big() - lo.to_big() + 1u32).ok()
        }
        SetRepr::Subset(b) => {
            let n = quiet_component_size(b)?;
            (n < 128).then(|| 1u128 << n)
        }
        SetRepr::FcnSet(d, r) => {
            let dn = quiet_component_size(d)?;
            if dn == 0 {
                return Some(1);
            }
            let rn = quiet_component_size(r)?;
            rn.checked_pow(u32::try_from(dn).ok()?)
        }
        SetRepr::RcdSet(fs) => product_size(fs.iter().map(|(_, v)| v)),
        SetRepr::Product(fs) => product_size(fs.iter()),
        SetRepr::Seq(r) => (quiet_component_size(r)? == 0).then_some(1),
        SetRepr::Nat | SetRepr::Int | SetRepr::Strings => None,
    }
}

fn product_size<'a>(comps: impl Iterator<Item = &'a Value>) -> Option<u128> {
    let sizes: Vec<Option<u128>> = comps.map(quiet_component_size).collect();
    if sizes.contains(&Some(0)) {
        return Some(0);
    }
    sizes
        .into_iter()
        .try_fold(1u128, |acc, n| acc.checked_mul(n?))
}

/// TLC's `Value.size()` for a component of a lazy set (a function's size is its
/// domain's, as in TLC).
fn value_size(v: &Value, ctx: Ctx) -> Result<u64> {
    match v {
        Value::Set(s) => size(s, ctx),
        Value::Fcn(f) => Ok(f.len() as u64),
        _ => {
            let what = match v {
                Value::Bool(_) => "boolean",
                Value::Int(_) | Value::BigInt(_) => "integer",
                Value::Str(_) => "string",
                _ => "model value",
            };
            Err(ValueError::new(
                ErrorKind::Incomparable,
                format!(
                    "Attempted to compute the number of elements in the {what} {}.",
                    p(v, ctx)
                ),
            ))
        }
    }
}

/// TLC's `size()` of `s`, with TLC's overflow errors in [`IntMode::Tlc`].
#[allow(
    clippy::too_many_lines,
    reason = "one arm per TLC size() method, kept together for comparison"
)]
pub(crate) fn size(s: &Set, ctx: Ctx) -> Result<u64> {
    let me = Value::Set(s.clone());
    let tlc = ctx.int_mode == IntMode::Tlc;
    let int_max = u64::try_from(i32::MAX).unwrap_or(u64::MAX);
    match s.repr() {
        SetRepr::Enum | SetRepr::Raw(_) | SetRepr::Deferred(_) => Ok(s.elements(ctx)?.len() as u64),
        SetRepr::Interval(lo, hi) => {
            if hi < lo {
                return Ok(0);
            }
            let n = u64::try_from(hi.to_big() - lo.to_big() + 1u32).ok();
            if tlc && n.is_none_or(|n| n > int_max) {
                return Err(ValueError::new(
                    ErrorKind::TooLarge,
                    format!(
                        "Size of interval value exceeds the maximum representable size (32bits): {}.",
                        ppr(&format!("{lo}..{hi}"))
                    ),
                ));
            }
            n.ok_or_else(|| too_large(&me, None, ctx))
        }
        SetRepr::Subset(b) => {
            let n = value_size(b, ctx)?;
            if tlc && n >= 31 {
                return Err(ValueError::new(
                    ErrorKind::Overflow,
                    format!(
                        "Overflow when computing the number of elements in:\n{}",
                        p(&me, ctx)
                    ),
                ));
            }
            u32::try_from(n)
                .ok()
                .and_then(|n| 1u64.checked_shl(n))
                .ok_or_else(|| too_large(&me, None, ctx))
        }
        SetRepr::FcnSet(d, r) => {
            if is_empty(d, ctx)? {
                return Ok(1);
            }
            if is_empty(r, ctx)? {
                return Ok(0);
            }
            let rsz = value_size(r, ctx)?;
            if rsz == 1 {
                return Ok(1);
            }
            let dsz = value_size(d, ctx)?;
            let mut sz: u64 = 1;
            for _ in 0..dsz {
                match sz.checked_mul(rsz) {
                    Some(n) if !tlc || n <= int_max => sz = n,
                    _ if tlc => {
                        return Err(ValueError::new(
                            ErrorKind::TooLarge,
                            format!(
                                "Overflow when computing the number of elements in:\n{}",
                                p(&me, ctx)
                            ),
                        ));
                    }
                    _ => return Err(too_large(&me, None, ctx)),
                }
            }
            Ok(sz)
        }
        SetRepr::RcdSet(_) | SetRepr::Product(_) => {
            if is_empty(&me, ctx)? {
                return Ok(0);
            }
            let comps: Vec<&Value> = match s.repr() {
                SetRepr::RcdSet(fs) => fs.iter().map(|(_, v)| v).collect(),
                SetRepr::Product(fs) => fs.iter().collect(),
                _ => Vec::new(),
            };
            let mut sz: u64 = 1;
            for c in comps {
                let n = value_size(c, ctx)?;
                match sz.checked_mul(n) {
                    Some(m) if !tlc || m <= int_max => sz = m,
                    _ if tlc => {
                        return Err(match s.repr() {
                            SetRepr::RcdSet(_) => ValueError::new(
                                ErrorKind::Overflow,
                                format!(
                                    "Overflow when computing the number of elements in:\n{}",
                                    p(&me, ctx)
                                ),
                            ),
                            _ => ValueError::new(
                                ErrorKind::TooLarge,
                                format!(
                                    "Overflow when computing the number of elements in {}",
                                    p(&me, ctx)
                                ),
                            ),
                        });
                    }
                    _ => return Err(too_large(&me, None, ctx)),
                }
            }
            Ok(sz)
        }
        SetRepr::Nat | SetRepr::Int | SetRepr::Strings => Err(not_enumerable(&me, ctx)),
        SetRepr::Seq(r) => {
            if is_empty(r, ctx)? {
                Ok(1)
            } else {
                Err(not_enumerable(&me, ctx))
            }
        }
    }
}

/// Materializes `s` in normalized order, without changing anything observable. The
/// caller ([`Set::quiet_elements`]) caches successes.
pub(crate) fn enumerate(s: &Set, ctx: Ctx) -> Result<Vec<Value>> {
    let ctx = Ctx {
        order: s.order(),
        ..ctx
    };
    match s.repr() {
        SetRepr::Enum => Ok(Vec::new()),
        SetRepr::Raw(raw) => crate::order::normalize_set(raw.to_vec(), ctx),
        SetRepr::Deferred(src) => crate::order::normalize_set(src.elements(ctx)?, ctx),
        _ => {
            let me = Value::Set(s.clone());
            match quiet_size(s) {
                Some(n) if n <= u128::from(MAX_ENUMERATION) => {}
                Some(n) => return Err(too_large(&me, Some(n), ctx)),
                None if is_infinite(s) => return Err(not_enumerable(&me, ctx)),
                None => {
                    // Not countable without enumerating a component: let the stream
                    // report why (an unenumerable component), or the size is too big.
                    let _ = stream(s, ctx, false)?;
                    return Err(too_large(&me, None, ctx));
                }
            }
            Ok(stream(s, ctx, false)?.collect())
        }
    }
}
