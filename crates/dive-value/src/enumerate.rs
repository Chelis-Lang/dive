//! Enumeration of lazy sets, in normalized order, and set sizes.
//!
//! Each lazy set is generated directly in the order normalization would produce
//! (as TLC's `toSetEnum` does): `SUBSET S` by size and then lexicographically,
//! `[S -> T]`, `S \X T` and `[a : S]` as odometers whose last position varies fastest.
//! Sizes follow TLC's `size()` methods, including its 32-bit overflow errors in
//! [`IntMode::Tlc`].

use crate::ctx::{Ctx, IntMode};
use crate::error::{ErrorKind, Result, ValueError};
use crate::int::Int;
use crate::print::{ppr, to_tlc_string_lenient};
use crate::tlc::is_empty;
use crate::value::{Fcn, Set, SetRepr, Value};

/// The most elements dive will materialize in one set.
pub const MAX_ENUMERATION: u64 = i32::MAX as u64;

fn p(v: &Value, ctx: Ctx) -> String {
    ppr(&to_tlc_string_lenient(v, ctx))
}

fn too_large(what: &Value, n: Option<u64>, ctx: Ctx) -> ValueError {
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

pub(crate) fn not_enumerable(what: &Value, ctx: Ctx) -> ValueError {
    ValueError::new(
        ErrorKind::NotEnumerable,
        format!(
            "Attempted to enumerate the infinite or non-enumerable set\n{}",
            p(what, ctx)
        ),
    )
}

/// Whether a set is infinite. Conservative only for sets built from infinite parts.
pub(crate) fn is_infinite(s: &Set) -> bool {
    let inf = |v: &Value| matches!(v, Value::Set(t) if is_infinite(t));
    match s.repr() {
        SetRepr::Nat | SetRepr::Int | SetRepr::Strings => true,
        SetRepr::Seq(r) => {
            !matches!(r, Value::Set(t) if !is_infinite(t) && t.elements(Ctx::default()).is_ok_and(|e| e.is_empty()))
        }
        SetRepr::Subset(b) => inf(b),
        SetRepr::FcnSet(d, r) => inf(d) || inf(r),
        SetRepr::RcdSet(fs) => fs.iter().any(|(_, v)| inf(v)),
        SetRepr::Product(fs) => fs.iter().any(inf),
        _ => false,
    }
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

fn component_elements(v: &Value, ctx: Ctx) -> Result<std::sync::Arc<[Value]>> {
    match v {
        Value::Set(s) => s.elements(ctx),
        _ => Err(ValueError::new(
            ErrorKind::NotEnumerable,
            format!("Attempted to enumerate the non-set value\n{}", p(v, ctx)),
        )),
    }
}

/// The number of elements of `s`.
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
            let n = hi.to_big() - lo.to_big() + 1u32;
            let n = u64::try_from(n).ok();
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
                let next = sz.checked_mul(rsz);
                match next {
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

/// Enumerates `s` in normalized order.
pub(crate) fn enumerate(s: &Set, ctx: Ctx) -> Result<Vec<Value>> {
    let ctx = Ctx {
        order: s.order(),
        ..ctx
    };
    let me = Value::Set(s.clone());
    match s.repr() {
        SetRepr::Enum => Ok(Vec::new()),
        SetRepr::Raw(raw) => crate::order::normalize_set(raw.to_vec(), ctx),
        SetRepr::Deferred(src) => crate::order::normalize_set(src.elements(ctx)?, ctx),
        SetRepr::Nat | SetRepr::Int | SetRepr::Strings => Err(not_enumerable(&me, ctx)),
        SetRepr::Seq(r) => {
            if is_empty(r, ctx)? {
                Ok(vec![Value::tuple(Vec::new())])
            } else {
                Err(not_enumerable(&me, ctx))
            }
        }
        _ => {
            let n = size(s, ctx)?;
            if n > MAX_ENUMERATION {
                return Err(too_large(&me, Some(n), ctx));
            }
            enumerate_lazy(s, ctx, usize::try_from(n).unwrap_or(usize::MAX))
        }
    }
}

fn enumerate_lazy(s: &Set, ctx: Ctx, n: usize) -> Result<Vec<Value>> {
    let mut out = Vec::with_capacity(n);
    match s.repr() {
        SetRepr::Interval(lo, hi) => {
            if let (Some(l), Some(h)) = (lo.to_i64(), hi.to_i64()) {
                out.extend((l..=h).map(Value::Int));
            } else {
                let mut i = lo.clone();
                while &i <= hi {
                    out.push(Value::int(i.clone()));
                    i = i.add(&Int::ONE, IntMode::Big)?;
                }
            }
        }
        SetRepr::Subset(b) => {
            let base = component_elements(b, ctx)?;
            let m = base.len();
            for k in 0..=m {
                // Combinations of k indices in lexicographic order.
                let mut idx: Vec<usize> = (0..k).collect();
                loop {
                    let elems = idx.iter().map(|&i| base[i].clone()).collect();
                    out.push(Value::Set(Set::from_sorted(ctx.order, elems)));
                    // Advance the rightmost index that can still move right.
                    let Some(pos) = (0..k).rev().find(|&i| idx[i] < m - k + i) else {
                        break;
                    };
                    idx[pos] += 1;
                    for j in pos + 1..k {
                        idx[j] = idx[j - 1] + 1;
                    }
                }
            }
        }
        SetRepr::FcnSet(d, r) => {
            if is_empty(d, ctx)? {
                return Ok(vec![Value::tuple(Vec::new())]);
            }
            if is_empty(r, ctx)? {
                return Ok(Vec::new());
            }
            let dom = component_elements(d, ctx)?;
            let rng = component_elements(r, ctx)?;
            // The function shape depends only on the domain.
            let template = crate::order::make_fcn(
                dom.iter().map(|x| (x.clone(), Value::FALSE)).collect(),
                ctx,
            )?;
            odometer(&vec![rng; dom.len()], |vals| {
                out.push(rebuild(&template, &dom, vals));
            });
        }
        SetRepr::RcdSet(fs) => {
            let mut comps = Vec::with_capacity(fs.len());
            for (_, v) in fs {
                comps.push(component_elements(v, ctx)?);
            }
            odometer(&comps, |vals| {
                let fields = fs
                    .iter()
                    .map(|(a, _)| a.clone())
                    .zip(vals.iter().cloned())
                    .collect();
                out.push(Value::Fcn(Fcn::record(fields)));
            });
        }
        SetRepr::Product(fs) => {
            let mut comps = Vec::with_capacity(fs.len());
            for v in fs {
                comps.push(component_elements(v, ctx)?);
            }
            odometer(&comps, |vals| out.push(Value::tuple(vals.to_vec())));
        }
        _ => {}
    }
    Ok(out)
}

/// Rebuilds a function with `template`'s domain shape and the values `vals`.
fn rebuild(template: &Value, dom: &[Value], vals: &[Value]) -> Value {
    match template.as_fcn().map(Fcn::kind) {
        Some(crate::FcnKind::Tuple) => Value::tuple(vals.to_vec()),
        Some(crate::FcnKind::Record) => Value::Fcn(Fcn::record(
            dom.iter()
                .zip(vals)
                .filter_map(|(d, v)| match d {
                    Value::Str(a) => Some((a.clone(), v.clone())),
                    _ => None,
                })
                .collect(),
        )),
        _ => Value::Fcn(Fcn::general(
            dom.iter().cloned().zip(vals.iter().cloned()).collect(),
        )),
    }
}

/// Visits every combination of one element from each component, the last component
/// varying fastest. Visits nothing if a component is empty.
fn odometer(comps: &[std::sync::Arc<[Value]>], mut visit: impl FnMut(&[Value])) {
    if comps.iter().any(|c| c.is_empty()) {
        return;
    }
    let mut idx = vec![0usize; comps.len()];
    let mut cur: Vec<Value> = comps.iter().map(|c| c[0].clone()).collect();
    loop {
        visit(&cur);
        let mut pos = comps.len();
        loop {
            if pos == 0 {
                return;
            }
            pos -= 1;
            idx[pos] += 1;
            if idx[pos] < comps[pos].len() {
                cur[pos] = comps[pos][idx[pos]].clone();
                break;
            }
            idx[pos] = 0;
            cur[pos] = comps[pos][0].clone();
        }
    }
}
