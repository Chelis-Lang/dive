//! TLC's JSON form of values, as written by `-dumpTrace json` and `JsonSerialize`.
//!
//! Port of `tlc2.module.Json.getNode` (tlaplus `94d0c50`) with Gson's compact
//! serialization (`JsonElement.toString`, no HTML escaping):
//! - records are objects, in field order;
//! - tuples and sequences (functions with domain `1..n`) are arrays;
//! - other functions are objects keyed by the domain element's TLC string form
//!   (a string domain element without quotes);
//! - finite sets are arrays in normalized order (intervals and other lazy sets are
//!   enumerated);
//! - strings and model values are JSON strings, integers numbers, booleans booleans.

use std::fmt::Write as _;

use crate::ctx::Ctx;
use crate::error::{ErrorKind, Result, ValueError};
use crate::print::to_tlc_string;
use crate::value::{FcnRepr, SetRepr, Value};

/// The JSON text TLC writes for a value.
///
/// ```
/// use dive_value::{Ctx, Interner, Value, json::to_tlc_json};
/// let ctx = Ctx::tlc();
/// let names = Interner::new();
/// let r = Value::record(vec![(names.intern("x"), Value::interval(1.into(), 3.into(), ctx))], ctx)?;
/// assert_eq!(to_tlc_json(&r, ctx)?, r#"{"x":[1,2,3]}"#);
/// # Ok::<(), dive_value::ValueError>(())
/// ```
///
/// # Errors
/// [`ErrorKind::Json`] for infinite sets (TLC: `Cannot convert value: unsupported
/// value type tlc2.value.impl.UserValue`), and enumeration errors.
pub fn to_tlc_json(v: &Value, ctx: Ctx) -> Result<String> {
    let mut out = String::new();
    write_json(v, ctx, &mut out)?;
    Ok(out)
}

/// Gson's string escaping (`JsonWriter.string` without HTML-safe mode).
#[must_use]
pub fn json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\t' => out.push_str("\\t"),
            '\u{0008}' => out.push_str("\\b"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\u{000C}' => out.push_str("\\f"),
            '\u{2028}' => out.push_str("\\u2028"),
            '\u{2029}' => out.push_str("\\u2029"),
            c if u32::from(c) < 0x20 => {
                let _ = write!(out, "\\u{:04x}", u32::from(c));
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

fn write_json(v: &Value, ctx: Ctx, out: &mut String) -> Result<()> {
    match v {
        Value::Bool(b) => out.push_str(if *b { "true" } else { "false" }),
        Value::Int(i) => {
            let _ = write!(out, "{i}");
        }
        Value::BigInt(b) => {
            let _ = write!(out, "{b}");
        }
        Value::Str(a) | Value::Model(a) => out.push_str(&json_string(a.as_str())),
        Value::Fcn(f) => {
            if let FcnRepr::Tuple(e) = &f.0.repr {
                out.push('[');
                for (i, x) in e.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    write_json(x, ctx, out)?;
                }
                out.push(']');
            } else {
                out.push('{');
                for (i, (d, x)) in f.iter().enumerate() {
                    if i > 0 {
                        out.push(',');
                    }
                    let key = match &d {
                        Value::Str(a) => a.as_str().to_owned(),
                        _ => to_tlc_string(&d, ctx)?,
                    };
                    out.push_str(&json_string(&key));
                    out.push(':');
                    write_json(x, ctx, out)?;
                }
                out.push('}');
            }
        }
        Value::Set(s) => {
            if matches!(
                s.repr(),
                SetRepr::Nat | SetRepr::Int | SetRepr::Strings | SetRepr::Seq(_)
            ) {
                return Err(ValueError::new(
                    ErrorKind::Json,
                    "Cannot convert value: unsupported value type tlc2.value.impl.UserValue",
                ));
            }
            out.push('[');
            for (i, x) in s.elements(ctx)?.iter().enumerate() {
                if i > 0 {
                    out.push(',');
                }
                write_json(x, ctx, out)?;
            }
            out.push(']');
        }
    }
    Ok(())
}
