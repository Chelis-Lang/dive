//! Legacy normalized JSON values (trace format version 1).
//!
//! TLC's `-dumpTrace json` form ([`dive_value::json`]) cannot be read back
//! exactly: a set and a sequence are both arrays, a string and a model value
//! are both strings. The legacy form distinguishes these common categories, but
//! does not preserve every stored representation. See [`crate::value_repr`] for
//! bounded opt-in version-2 transport. Scalars remain plain JSON:
//!
//! | Value | JSON |
//! |---|---|
//! | `TRUE`, `FALSE` | `true`, `false` |
//! | an integer in `i64` range | a number: `3`, `-7` |
//! | any other integer | `{"int": "123456789012345678901234567890"}` |
//! | a string `"a"` | `"a"` |
//! | a model value `m1` | `{"model": "m1"}` |
//! | a set `{1, 2}` | `{"set": [1, 2]}` |
//! | a tuple or sequence `<<1, "a">>` | `{"seq": [1, "a"]}` |
//! | a record `[a \|-> 1]` | `{"record": {"a": 1}}` |
//! | any other function `(1 :> "a" @@ 3 :> "b")` | `{"fcn": [[1, "a"], [3, "b"]]}` |
//!
//! A function whose domain is `1..n` is a `seq`; one whose domain is a
//! non-empty set of strings is a `record`. Reading normalizes, so a `fcn`
//! with domain `{1, 2}` reads as the sequence it equals. Sets are written
//! in the run's normalized order. Reading runs the existing constructor, so
//! mixed structured sets may refuse after serialization (#190); input order
//! is not universally interchangeable. Literal/function and Raw/normalized
//! representation flags are not preserved. Infinite sets (`Nat`) have no form.

use dive_value::{Ctx, Int, Interner, Value};
use serde_json::{Map, Value as J, json};

/// Writes `v` in dive's JSON form.
///
/// # Errors
/// For an infinite set, or a lazy set that cannot be enumerated.
pub fn to_json(v: &Value, ctx: Ctx) -> Result<J, String> {
    Ok(match v {
        Value::Bool(b) => J::Bool(*b),
        Value::Int(i) => J::from(*i),
        Value::BigInt(b) => json!({ "int": b.to_string() }),
        Value::Str(a) => J::String(a.as_str().to_owned()),
        Value::Model(a) => json!({ "model": a.as_str() }),
        Value::Set(s) => {
            if s.is_infinite() {
                return Err("an infinite set has no JSON form".to_owned());
            }
            let elems = s.elements(ctx).map_err(|e| e.message().to_owned())?;
            let items = elems
                .iter()
                .map(|e| to_json(e, ctx))
                .collect::<Result<Vec<_>, _>>()?;
            json!({ "set": items })
        }
        Value::Fcn(f) => {
            if let Some(t) = f.as_tuple() {
                let items = t
                    .iter()
                    .map(|e| to_json(e, ctx))
                    .collect::<Result<Vec<_>, _>>()?;
                json!({ "seq": items })
            } else if let Some(r) = f.as_record() {
                let mut m = Map::new();
                for (k, x) in r {
                    m.insert(k.as_str().to_owned(), to_json(x, ctx)?);
                }
                json!({ "record": m })
            } else {
                let items = f
                    .iter()
                    .map(|(d, x)| Ok(J::Array(vec![to_json(&d, ctx)?, to_json(x, ctx)?])))
                    .collect::<Result<Vec<_>, String>>()?;
                json!({ "fcn": items })
            }
        }
    })
}

/// Reads a value in dive's JSON form, interning names in `names`.
///
/// # Errors
/// A message naming what is malformed.
pub fn from_json(j: &J, names: &Interner, ctx: Ctx) -> Result<Value, String> {
    match j {
        J::Bool(b) => Ok(Value::Bool(*b)),
        J::Number(n) => n.as_i64().map(Value::Int).ok_or_else(|| {
            format!("`{n}` is not an integer in i64 range (write {{\"int\": \"...\"}})")
        }),
        J::String(s) => Ok(Value::string(names, s)),
        J::Null => Err("null is not a value".to_owned()),
        J::Array(_) => {
            Err("a bare array is ambiguous: write {\"set\": [...]} or {\"seq\": [...]}".to_owned())
        }
        J::Object(m) => {
            let mut it = m.iter();
            let (Some((tag, body)), None) = (it.next(), it.next()) else {
                return Err(format!(
                    "a value object has exactly one key (set, seq, record, fcn, model or int), got {}",
                    m.len()
                ));
            };
            match (tag.as_str(), body) {
                ("model", J::String(s)) => Ok(Value::model(names, s)),
                ("int", J::String(s)) => parse_int(s, ctx),
                ("set", J::Array(a)) => {
                    let elems = a
                        .iter()
                        .map(|e| from_json(e, names, ctx))
                        .collect::<Result<Vec<_>, _>>()?;
                    Value::set(elems, ctx).map_err(|e| e.message().to_owned())
                }
                ("seq", J::Array(a)) => Ok(Value::tuple(
                    a.iter()
                        .map(|e| from_json(e, names, ctx))
                        .collect::<Result<Vec<_>, _>>()?,
                )),
                ("record", J::Object(fields)) => {
                    let fields = fields
                        .iter()
                        .map(|(k, x)| Ok((names.intern(k), from_json(x, names, ctx)?)))
                        .collect::<Result<Vec<_>, String>>()?;
                    Value::record(fields, ctx).map_err(|e| e.message().to_owned())
                }
                ("fcn", J::Array(pairs)) => {
                    let pairs = pairs
                        .iter()
                        .map(|p| match p.as_array().map(Vec::as_slice) {
                            Some([d, x]) => {
                                Ok((from_json(d, names, ctx)?, from_json(x, names, ctx)?))
                            }
                            _ => Err("a fcn entry is a [argument, value] pair".to_owned()),
                        })
                        .collect::<Result<Vec<_>, String>>()?;
                    Value::fcn(pairs, ctx).map_err(|e| e.message().to_owned())
                }
                (tag, _) => Err(format!(
                    "`{tag}` is not a value tag, or its body has the wrong type"
                )),
            }
        }
    }
}

/// An integer written as a decimal string, in the run's integer mode.
fn parse_int(s: &str, ctx: Ctx) -> Result<Value, String> {
    let (neg, digits) = match s.strip_prefix('-') {
        Some(d) => (true, d),
        None => (false, s),
    };
    if digits.is_empty() || !digits.bytes().all(|b| b.is_ascii_digit()) {
        return Err(format!("`{s}` is not a decimal integer"));
    }
    let i = Int::from_literal(digits, ctx.int_mode).map_err(|e| e.message().to_owned())?;
    let i = if neg {
        i.neg(ctx.int_mode).map_err(|e| e.message().to_owned())?
    } else {
        i
    };
    Ok(Value::int(i))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips_every_kind() -> Result<(), String> {
        let ctx = Ctx::dive();
        let names = Interner::new();
        let text = r#"{"record":{"a":{"set":[1,2]},"b":{"seq":["x",{"model":"m1"}]},
            "c":{"fcn":[[{"model":"m1"},true],[{"model":"m2"},false]]},
            "d":{"int":"123456789012345678901234567890"},"e":{"seq":[]}}}"#;
        let j: J = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let v = from_json(&j, &names, ctx)?;
        let back = to_json(&v, ctx)?;
        assert_eq!(back, j);
        assert_eq!(from_json(&back, &names, ctx)?, v);
        Ok(())
    }

    #[test]
    fn rejects_ambiguous_and_malformed_input() {
        let ctx = Ctx::dive();
        let names = Interner::new();
        for bad in [
            "[1, 2]",
            "null",
            "1.5",
            r#"{"set": [1], "seq": []}"#,
            r#"{"bag": []}"#,
            r#"{"int": "12x"}"#,
        ] {
            let j: J = serde_json::from_str(bad).unwrap_or(J::Null);
            assert!(from_json(&j, &names, ctx).is_err(), "{bad}");
        }
    }

    #[test]
    fn a_fcn_over_one_to_n_reads_as_the_sequence() -> Result<(), String> {
        let ctx = Ctx::tlc();
        let names = Interner::new();
        let j: J =
            serde_json::from_str(r#"{"fcn": [[2, "b"], [1, "a"]]}"#).map_err(|e| e.to_string())?;
        let v = from_json(&j, &names, ctx)?;
        assert_eq!(
            v,
            Value::tuple(vec![Value::string(&names, "a"), Value::string(&names, "b")])
        );
        Ok(())
    }
}
