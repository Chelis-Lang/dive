//! TLC's value printing.
//!
//! [`to_tlc_string`] ports the `toString` methods of `tlc2.value.impl` (tlaplus
//! `94d0c50`), including which lazy sets TLC expands when printing:
//! - `SUBSET S` when `|S| < 7`;
//! - `[S -> T]`, `[a : S]` and `S \X T` when they have fewer than 2000 elements
//!   (`TLCGlobals.enumBound`);
//! - filters and maps always;
//! - intervals never (`1..3`, or `{}` when empty).
//!
//! [`print_value`] adds TLC's pretty-printer ([`ppr`], a port of `tlc2.pprint`), as
//! `Print`, `PrintT`, error messages and counterexample states use it.

#![allow(
    clippy::many_single_char_names,
    reason = "ports keep short operand names (a, b, s, t, x, y)"
)]

use std::fmt::Write as _;

use crate::atom::Atom;
use crate::ctx::Ctx;
use crate::enumerate;
use crate::error::Result;
use crate::stream::elems_in;
use crate::value::{FcnRepr, SetRepr, Value};

pub use crate::pprint::{WIDTH, ppr, ppr_width};

/// `Value.toStringUnchecked()`: the single-line form, with errors raised while
/// deciding how to print a lazy set propagated (as in `Print` and `ToString`).
///
/// ```
/// use dive_value::{Ctx, Interner, Value, print::to_tlc_string};
/// let ctx = Ctx::tlc();
/// let names = Interner::new();
/// let v = Value::set(vec![Value::from(2), Value::from(1)], ctx)?;
/// assert_eq!(to_tlc_string(&v, ctx)?, "{1, 2}");
/// let s = Value::string(&names, "a\"b");
/// assert_eq!(to_tlc_string(&s, ctx)?, r#""a\"b""#);
/// # Ok::<(), dive_value::ValueError>(())
/// ```
///
/// # Errors
/// Normalization errors of unnormalized sets, and errors raised while sizing or
/// enumerating a lazy set that TLC would expand.
pub fn to_tlc_string(v: &Value, ctx: Ctx) -> Result<String> {
    let mut out = String::new();
    write_value(
        v,
        ctx,
        Mode {
            swallow: false,
            mark: true,
        },
        &mut out,
    )?;
    Ok(out)
}

/// `Value.toString()`: like [`to_tlc_string`], but errors raised while deciding whether
/// to expand a lazy set are swallowed (the set is printed in its lazy form), as TLC
/// does when building error messages. Never fails; an unprintable value prints as a
/// placeholder naming its kind.
#[must_use]
pub fn to_tlc_string_lenient(v: &Value, ctx: Ctx) -> String {
    let mut out = String::new();
    match write_value(
        v,
        ctx,
        Mode {
            swallow: true,
            mark: true,
        },
        &mut out,
    ) {
        Ok(()) => out,
        Err(_) => format!("<unprintable {}>", v.kind_name()),
    }
}

/// What `Print`/`PrintT` print for a value: `Values.ppr(v.toStringUnchecked())`.
///
/// # Errors
/// As [`to_tlc_string`].
pub fn print_value(v: &Value, ctx: Ctx) -> Result<String> {
    Ok(ppr(&to_tlc_string(v, ctx)?))
}

/// A state as TLC prints it in a counterexample (`TLCStateMut.toString`): one
/// `/\ x = value` line per variable, or `x = value` when there is a single variable.
/// Each value is pretty-printed on its own, at width 80.
///
/// # Errors
/// As [`to_tlc_string`].
pub fn format_state(vars: &[(&str, &Value)], ctx: Ctx) -> Result<String> {
    let mut out = String::new();
    if let [(name, v)] = vars {
        let _ = writeln!(out, "{name} = {}", ppr(&to_tlc_string(v, ctx)?));
        return Ok(out);
    }
    for (name, v) in vars {
        let _ = writeln!(out, "/\\ {name} = {}", ppr(&to_tlc_string(v, ctx)?));
    }
    Ok(out)
}

/// `StringValue.PrintVersion`: escapes `"`, `\`, tab, newline, form feed and carriage
/// return.
#[must_use]
pub fn escape_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\n' => out.push_str("\\n"),
            '\u{000C}' => out.push_str("\\f"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out
}

/// `FcnRcdValue.isName`: whether a string field prints in record syntax.
fn is_name(s: &str) -> bool {
    let mut has_letter = false;
    let mut len = 0usize;
    for u in s.encode_utf16() {
        len += 1;
        if u == u16::from(b'_') {
            continue;
        }
        let Some(c) = char::from_u32(u32::from(u)) else {
            // A lone surrogate is neither a letter nor a digit in Java.
            return false;
        };
        if !java_is_letter_or_digit(c) {
            return false;
        }
        has_letter |= java_is_letter(c);
    }
    has_letter && (len < 4 || !(s.starts_with("WF_") || s.starts_with("SF_")))
}

/// `Character.isLetter`, exact for ASCII; Unicode's alphabetic property otherwise.
pub(crate) fn java_is_letter(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphabetic()
    } else {
        c.is_alphabetic()
    }
}

/// `Character.isLetterOrDigit`, exact for ASCII; Unicode's alphanumeric property
/// otherwise.
pub(crate) fn java_is_letter_or_digit(c: char) -> bool {
    if c.is_ascii() {
        c.is_ascii_alphanumeric()
    } else {
        c.is_alphanumeric()
    }
}

/// How a value is being printed.
#[derive(Clone, Copy)]
struct Mode {
    /// Swallow errors raised while deciding how to print a lazy set (`toString`).
    swallow: bool,
    /// Print as TLC does, normalizing unnormalized sets as a side effect. Off only
    /// for `Debug`, which must not change anything observable.
    mark: bool,
}

/// The `Debug` form: TLC's printing, without normalizing anything and without
/// failing on what TLC would expand.
pub(crate) fn to_debug_string(v: &Value) -> Option<String> {
    let mut out = String::new();
    let mode = Mode {
        swallow: true,
        mark: false,
    };
    write_value(v, Ctx::default(), mode, &mut out).ok()?;
    Some(out)
}

fn write_list<'a>(
    items: impl Iterator<Item = &'a Value>,
    ctx: Ctx,
    mode: Mode,
    out: &mut String,
) -> Result<()> {
    for (i, v) in items.enumerate() {
        if i > 0 {
            out.push_str(", ");
        }
        write_value(v, ctx, mode, out)?;
    }
    Ok(())
}

/// The size estimate TLC uses to decide whether to expand `[S -> T]`.
fn fcn_set_bound(d: &Value, r: &Value, ctx: Ctx, mode: Mode) -> Result<u128> {
    if crate::tlc::is_empty(d, ctx)? {
        return Ok(1);
    }
    let dsz = component_size(d, ctx, mode)?;
    let rsz = u128::from(component_size(r, ctx, mode)?);
    let mut sz: u128 = 1;
    let mut i = 0;
    while i < dsz && sz <= 2_147_483_647 {
        sz *= rsz;
        i += 1;
    }
    Ok(sz)
}

/// The size estimate TLC uses to decide whether to expand `[a : S]` and `S \X T`.
fn product_bound<'a>(comps: impl Iterator<Item = &'a Value>, ctx: Ctx, mode: Mode) -> Result<u128> {
    let mut sz: u128 = 1;
    for c in comps {
        let n = u128::from(component_size(c, ctx, mode)?);
        if n == 0 {
            return Ok(0);
        }
        if sz <= 2_147_483_647 {
            sz *= n;
        }
    }
    Ok(sz)
}

fn component_size(v: &Value, ctx: Ctx, mode: Mode) -> Result<u64> {
    match v {
        Value::Set(s) if mode.mark => s.size(ctx),
        Value::Set(s) => enumerate::quiet_size(s)
            .and_then(|n| u64::try_from(n).ok())
            .ok_or_else(|| crate::stream::not_enumerable(v, ctx)),
        Value::Fcn(f) => Ok(f.len() as u64),
        _ => Err(crate::stream::not_enumerable(v, ctx)),
    }
}

/// TLC's `TLCGlobals.enumBound`.
const ENUM_BOUND: u128 = 2000;

#[allow(
    clippy::too_many_lines,
    reason = "one arm per TLC toString method, kept together for comparison"
)]
fn write_value(v: &Value, ctx: Ctx, mode: Mode, out: &mut String) -> Result<()> {
    let _guard = crate::depth::Guard::enter()?;
    match v {
        Value::Bool(b) => out.push_str(if *b { "TRUE" } else { "FALSE" }),
        Value::Int(_) | Value::BigInt(_) => {
            let _ = write!(out, "{}", v.as_int().unwrap_or(crate::Int::ZERO));
        }
        Value::Str(a) => {
            out.push('"');
            out.push_str(&escape_string(a.as_str()));
            out.push('"');
        }
        Value::Model(a) => out.push_str(a.as_str()),
        Value::Fcn(f) => write_fcn(f, ctx, mode, out)?,
        Value::Set(s) => write_set(s, ctx, mode, out)?,
    }
    Ok(())
}

#[inline(never)]
fn write_fcn(f: &crate::Fcn, ctx: Ctx, mode: Mode, out: &mut String) -> Result<()> {
    match &f.0.repr {
        FcnRepr::Tuple(e) => {
            out.push_str("<<");
            write_list(e.iter(), ctx, mode, out)?;
            out.push_str(">>");
        }
        // `RecordValue.toString` always uses record syntax; a string-domain
        // `FcnRcdValue` only when every field is a name (`isRcd`).
        FcnRepr::Record(fs)
            if f.is_record_literal() || fs.iter().all(|(a, _)| is_name(a.as_str())) =>
        {
            let mut fields: Vec<(Atom, Value)> = fs.to_vec();
            if f.order() != ctx.order {
                fields.sort_by(|a, b| a.0.cmp_with(&b.0, ctx.order));
            }
            out.push('[');
            for (i, (a, x)) in fields.iter().enumerate() {
                if i > 0 {
                    out.push_str(", ");
                }
                out.push_str(a.as_str());
                out.push_str(" |-> ");
                write_value(x, ctx, mode, out)?;
            }
            out.push(']');
        }
        _ => {
            let mut pairs: Vec<(Value, Value)> = f.iter().map(|(d, x)| (d, x.clone())).collect();
            if f.order() != ctx.order {
                pairs.sort_by(|a, b| a.0.cmp_with(&b.0, ctx.order));
            }
            out.push('(');
            for (i, (d, x)) in pairs.iter().enumerate() {
                if i > 0 {
                    out.push_str(" @@ ");
                }
                write_value(d, ctx, mode, out)?;
                out.push_str(" :> ");
                write_value(x, ctx, mode, out)?;
            }
            out.push(')');
        }
    }
    Ok(())
}

#[inline(never)]
#[allow(
    clippy::too_many_lines,
    reason = "one arm per TLC toString method of a set, kept together for comparison"
)]
fn write_set(s: &crate::Set, ctx: Ctx, mode: Mode, out: &mut String) -> Result<()> {
    let expand = |decide: Result<bool>| -> Result<bool> {
        match decide {
            Ok(b) => Ok(b),
            Err(_) if mode.swallow => Ok(false),
            Err(e) => Err(e),
        }
    };
    let enumerated = |out: &mut String| -> Result<()> {
        let elems = if mode.mark {
            elems_in(s, ctx, true)?
        } else {
            s.quiet_elements(ctx)?
        };
        out.push('{');
        write_list(elems.iter(), ctx, mode, out)?;
        out.push('}');
        Ok(())
    };
    match s.repr() {
        SetRepr::Enum | SetRepr::Raw(_) | SetRepr::Deferred(_) => enumerated(out)?,
        SetRepr::Interval(lo, hi) => {
            if lo <= hi {
                let _ = write!(out, "{lo}..{hi}");
            } else {
                out.push_str("{}");
            }
        }
        SetRepr::Subset(b) => {
            if expand(component_size(b, ctx, mode).map(|n| n < 7))? {
                enumerated(out)?;
            } else {
                out.push_str("SUBSET ");
                if matches!(b, Value::Set(t) if t.as_interval().is_some()) {
                    out.push('(');
                    write_value(b, ctx, mode, out)?;
                    out.push(')');
                } else {
                    write_value(b, ctx, mode, out)?;
                }
            }
        }
        SetRepr::FcnSet(d, r) => {
            let small = expand(fcn_set_bound(d, r, ctx, mode).map(|n| n < ENUM_BOUND))?;
            let printed = small && {
                let mut tmp = String::new();
                match enumerated(&mut tmp) {
                    Ok(()) => {
                        out.push_str(&tmp);
                        true
                    }
                    Err(_) if mode.swallow => false,
                    Err(e) => return Err(e),
                }
            };
            if !printed {
                out.push('[');
                write_value(d, ctx, mode, out)?;
                out.push_str(" -> ");
                write_value(r, ctx, mode, out)?;
                out.push(']');
            }
        }
        SetRepr::RcdSet(fs) => {
            if expand(product_bound(fs.iter().map(|(_, x)| x), ctx, mode).map(|n| n < ENUM_BOUND))?
            {
                enumerated(out)?;
            } else {
                out.push('[');
                for (i, (a, x)) in fs.iter().enumerate() {
                    if i > 0 {
                        out.push_str(", ");
                    }
                    out.push_str(a.as_str());
                    out.push_str(": ");
                    write_value(x, ctx, mode, out)?;
                }
                out.push(']');
            }
        }
        SetRepr::Product(fs) => {
            if expand(product_bound(fs.iter(), ctx, mode).map(|n| n < ENUM_BOUND))? {
                enumerated(out)?;
            } else if !fs.is_empty() {
                out.push('(');
                for (i, x) in fs.iter().enumerate() {
                    if i > 0 {
                        out.push_str(" \\X ");
                    }
                    write_value(x, ctx, mode, out)?;
                }
                out.push(')');
            }
        }
        SetRepr::Nat => out.push_str("Nat"),
        SetRepr::Int => out.push_str("Int"),
        SetRepr::Strings => out.push_str("STRING"),
        SetRepr::Seq(r) => {
            out.push_str("Seq(");
            write_value(r, ctx, mode, out)?;
            out.push(')');
        }
    }
    Ok(())
}
