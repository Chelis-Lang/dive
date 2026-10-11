//! Bounded stored-value JSON transport for opt-in trace version 2 (ADR 0055).
//!
//! Decoding checks representation through ordinary constructors. A normalization
//! witness is only a permutation of supplied elements: the decoder recomputes it
//! using the bound value order and requires the exact stored output. No receipt,
//! literal bit or normalized flag is accepted as a semantic certificate. Deferred
//! and symbolic sets remain unsupported; this does not change value comparisons.

use dive_value::{Ctx, FcnKind, Interner, SetKind, Value, ValueOrder};
use serde_json::{Value as J, json};

/// Maximum stored nodes in one transported value.
pub const MAX_NODES: usize = 16_384;
/// Maximum nesting depth in one transported value.
pub const MAX_DEPTH: usize = 32;
/// Maximum members in one compound value (bounds normalization work).
pub const MAX_MEMBERS: usize = 512;
/// Maximum conservative JSON byte charge in one value, including escaped text.
pub const MAX_BYTES: usize = 4 * 1024 * 1024;
/// Maximum decimal digits admitted before parsing a large integer.
pub const MAX_INTEGER_DIGITS: usize = 4096;
/// Number of deterministic normalization witnesses tried by the encoder.
pub const MAX_WITNESSES: usize = 2;
/// Maximum conservative normalization comparison-node work charge per value.
pub const MAX_NORMALIZATION_WORK: usize = 1_048_576;

#[derive(Default)]
struct Budget {
    nodes: usize,
    bytes: usize,
    work: usize,
}
impl Budget {
    fn node(&mut self, depth: usize) -> Result<(), String> {
        self.nodes += 1;
        self.charge(128)?;
        if depth > MAX_DEPTH || self.nodes > MAX_NODES {
            return Err("stored value node/depth limit".into());
        }
        Ok(())
    }
    fn charge(&mut self, bytes: usize) -> Result<(), String> {
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .ok_or("stored value byte limit")?;
        if self.bytes > MAX_BYTES {
            return Err("stored value byte limit".into());
        }
        Ok(())
    }
    fn normalization(
        &mut self,
        count: usize,
        max_nodes: usize,
        attempts: usize,
    ) -> Result<(), String> {
        let charge = count
            .saturating_mul(count)
            .saturating_mul(max_nodes)
            .saturating_mul(attempts);
        self.work = self.work.saturating_add(charge);
        if self.work > MAX_NORMALIZATION_WORK {
            return Err("stored normalization work admission limit".into());
        }
        Ok(())
    }
    fn text(&mut self, text: &str) -> Result<(), String> {
        self.charge(text.len().saturating_mul(6))
    }
}
fn members(n: usize) -> Result<(), String> {
    if n > MAX_MEMBERS {
        Err("stored value compound-member limit".into())
    } else {
        Ok(())
    }
}
fn integer_digits(text: &str) -> Result<(), String> {
    if text.strip_prefix('-').unwrap_or(text).len() > MAX_INTEGER_DIGITS {
        return Err("stored integer digit admission limit".into());
    }
    Ok(())
}
fn order_name(order: ValueOrder) -> &'static str {
    match order {
        ValueOrder::Canonical => "canonical",
        ValueOrder::Tlc => "tlc",
    }
}

/// Writes admitted stored representation without forcing providers or symbolic sets.
///
/// # Errors
/// Unsupported shapes, bounded traversal, or no proved normalization witness.
pub fn to_json(v: &Value, ctx: Ctx) -> Result<J, String> {
    let out = write(v, ctx, &mut Budget::default(), 0, true)?;
    preflight(&out, &mut Budget::default(), 0)?;
    Ok(out)
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive stored representation writer"
)]
fn write(
    v: &Value,
    ctx: Ctx,
    budget: &mut Budget,
    depth: usize,
    proofs: bool,
) -> Result<J, String> {
    budget.node(depth)?;
    Ok(match v {
        Value::Bool(b) => J::Bool(*b),
        Value::Int(i) => J::from(*i),
        Value::BigInt(i) => {
            if v.as_int().and_then(|n| n.to_i64()).is_some() {
                return Err(
                    "stored trace transport refuses noncanonical small BigInt representation"
                        .into(),
                );
            }
            let s = i.to_string();
            integer_digits(&s)?;
            budget.text(&s)?;
            json!({"int":s})
        }
        Value::Str(a) => {
            budget.text(a.as_str())?;
            J::String(a.as_str().into())
        }
        Value::Model(a) => {
            budget.text(a.as_str())?;
            json!({"model":a.as_str()})
        }
        Value::Fcn(f) => {
            if f.kind() != FcnKind::Tuple && f.order() != ctx.order {
                return Err("stored function order differs from bound profile".into());
            }
            members(f.len())?;
            if let Some(values) = f.as_tuple() {
                let items = values
                    .iter()
                    .map(|x| write(x, ctx, budget, depth + 1, proofs))
                    .collect::<Result<Vec<_>, _>>()?;
                json!({"repr":if f.is_tuple_literal(){"tuple"}else{"tuple-fcn"},"items":items})
            } else if let Some(fields) = f.as_record() {
                let mut items = Vec::with_capacity(fields.len());
                for (a, x) in fields {
                    budget.text(a.as_str())?;
                    items.push(json!([
                        a.as_str(),
                        write(x, ctx, budget, depth + 1, proofs)?
                    ]));
                }
                if !f.is_record_literal() {
                    charge_items(&items, budget, 1)?;
                }
                json!({"repr":if f.is_record_literal(){"record"}else{"record-fcn"},"order":order_name(f.order()),"items":items})
            } else {
                let mut items = Vec::with_capacity(f.len());
                for (d, x) in f.iter() {
                    items.push(json!([
                        write(&d, ctx, budget, depth + 1, proofs)?,
                        write(x, ctx, budget, depth + 1, proofs)?
                    ]));
                }
                charge_items(&items, budget, 1)?;
                json!({"repr":"fcn","order":order_name(f.order()),"items":items})
            }
        }
        Value::Set(s) => {
            let Some(values) = s.stored_elements() else {
                return Err("stored trace transport supports explicit sets only".into());
            };
            members(values.len())?;
            if s.order() != ctx.order {
                return Err("stored set order differs from bound profile".into());
            }
            let items = values
                .iter()
                .map(|x| write(x, ctx, budget, depth + 1, proofs))
                .collect::<Result<Vec<_>, _>>()?;
            if proofs {
                charge_items(
                    &items,
                    budget,
                    if s.kind() == SetKind::Enum {
                        MAX_WITNESSES
                    } else {
                        1
                    },
                )?;
            }
            match s.kind() {
                SetKind::Raw => {
                    json!({"repr":"raw-set","order":order_name(s.order()),"normalized":s.is_tlc_normalized(),"items":items})
                }
                SetKind::Enum => {
                    let mut j = json!({"repr":"set","order":order_name(s.order()),"items":items});
                    if proofs {
                        j["witness"] = json!(witness(values, ctx)?);
                    }
                    j
                }
                _ => return Err("stored trace transport supports explicit sets only".into()),
            }
        }
    })
}
fn charge_items(items: &[J], budget: &mut Budget, attempts: usize) -> Result<(), String> {
    let mut max_nodes = 1;
    for item in items {
        let mut b = Budget::default();
        preflight(item, &mut b, 0)?;
        max_nodes = max_nodes.max(b.nodes);
    }
    budget.normalization(items.len(), max_nodes, attempts)
}
fn shape(v: &Value, ctx: Ctx) -> Result<J, String> {
    write(v, ctx, &mut Budget::default(), 0, false)
}
fn exact_items(value: &Value, expected: &[Value], ctx: Ctx) -> Result<bool, String> {
    let Value::Set(s) = value else {
        return Ok(false);
    };
    let Some(actual) = s.stored_elements() else {
        return Ok(false);
    };
    if actual.len() != expected.len() {
        return Ok(false);
    }
    for (a, b) in actual.iter().zip(expected) {
        if shape(a, ctx)? != shape(b, ctx)? {
            return Ok(false);
        }
    }
    Ok(true)
}
fn witness(values: &[Value], ctx: Ctx) -> Result<Vec<usize>, String> {
    for reversed in [false, true] {
        let mut indices: Vec<usize> = (0..values.len()).collect();
        if reversed {
            indices.reverse();
        }
        let candidate =
            dive_value::codec::probe_set(indices.iter().map(|&i| values[i].clone()).collect(), ctx);
        if let Ok(candidate) = candidate
            && exact_items(&candidate, values, ctx)?
        {
            return Ok(indices);
        }
    }
    Err("no bounded normalization construction witness for stored set".into())
}

/// Reads stored representation under the receiving model's value context.
///
/// Every representation/order flag is checked using ordinary constructors. Names
/// must already be interned by the receiving model: input cannot reorder its table.
/// No callbacks or predicates are executed. Successful decoding proves shape only.
///
/// # Errors
/// Malformed tags, foreign names/order, forged flags/witnesses or resource limits.
pub fn from_json(j: &J, names: &Interner, ctx: Ctx) -> Result<Value, String> {
    preflight(j, &mut Budget::default(), 0)?;
    read(j, names, ctx, &mut Budget::default(), 0)
}
fn preflight(j: &J, budget: &mut Budget, depth: usize) -> Result<(), String> {
    if depth > MAX_DEPTH * 4 {
        return Err("stored JSON nesting limit".into());
    }
    budget.node(0)?;
    match j {
        J::String(s) => budget.text(s)?,
        J::Array(a) => {
            members(a.len())?;
            for x in a {
                preflight(x, budget, depth + 1)?;
            }
        }
        J::Object(o) => {
            members(o.len())?;
            for (k, v) in o {
                budget.text(k)?;
                preflight(v, budget, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}
fn keys(j: &J, expected: &[&str]) -> Result<(), String> {
    let o = j.as_object().ok_or("stored value requires an object")?;
    if o.len() != expected.len() || o.keys().any(|k| !expected.contains(&k.as_str())) {
        return Err("stored value has unexpected or missing fields".into());
    }
    Ok(())
}
fn checked_order(j: &J, ctx: Ctx) -> Result<(), String> {
    if j["order"].as_str() != Some(order_name(ctx.order)) {
        return Err("stored value order differs from bound profile".into());
    }
    Ok(())
}
fn items(j: &J) -> Result<&[J], String> {
    j["items"]
        .as_array()
        .map(Vec::as_slice)
        .ok_or("stored value items must be an array".into())
}
fn atom(names: &Interner, text: &str) -> Result<dive_value::Atom, String> {
    names
        .get(text)
        .ok_or_else(|| format!("stored value name `{text}` is not in receiving model"))
}
fn pair(j: &J) -> Result<(&J, &J), String> {
    match j.as_array().map(Vec::as_slice) {
        Some([a, b]) => Ok((a, b)),
        _ => Err("stored entry requires exactly two values".into()),
    }
}
#[allow(
    clippy::too_many_lines,
    reason = "exhaustive checked stored representation reader"
)]
fn read(
    j: &J,
    names: &Interner,
    ctx: Ctx,
    budget: &mut Budget,
    depth: usize,
) -> Result<Value, String> {
    budget.node(depth)?;
    match j {
        J::String(s) => return Ok(Value::Str(atom(names, s)?)),
        J::Bool(b) => return Ok(Value::Bool(*b)),
        J::Number(n) => {
            return n
                .as_i64()
                .map(Value::Int)
                .ok_or("stored integer requires i64 or decimal int tag".into());
        }
        J::Object(o) if o.contains_key("model") => {
            keys(j, &["model"])?;
            return Ok(Value::Model(atom(
                names,
                j["model"].as_str().ok_or("model name must be text")?,
            )?));
        }
        J::Object(o) if o.contains_key("int") => {
            keys(j, &["int"])?;
            integer_digits(
                j["int"]
                    .as_str()
                    .ok_or("stored decimal integer must be text")?,
            )?;
            let v = crate::value_json::from_json(j, names, ctx)?;
            if !matches!(v, Value::BigInt(_)) {
                return Err("stored decimal integer tag requires canonical large integer".into());
            }
            return Ok(v);
        }
        _ => {}
    }
    let kind = j["repr"].as_str().ok_or("stored value needs a repr tag")?;
    if matches!(kind, "set" | "raw-set" | "fcn" | "record-fcn") {
        charge_items(items(j)?, budget, 1)?;
    }
    let value = match kind {
        "tuple" | "tuple-fcn" => {
            keys(j, &["repr", "items"])?;
            let values = items(j)?
                .iter()
                .map(|x| read(x, names, ctx, budget, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            if kind == "tuple" {
                Value::tuple(values)
            } else {
                Value::tuple_fcn(values)
            }
        }
        "record" | "record-fcn" | "fcn" => {
            keys(j, &["repr", "order", "items"])?;
            checked_order(j, ctx)?;
            let mut pairs = Vec::new();
            for p in items(j)? {
                let (d, x) = pair(p)?;
                let d = if kind == "fcn" {
                    read(d, names, ctx, budget, depth + 1)?
                } else {
                    Value::Str(atom(names, d.as_str().ok_or("record field must be text")?)?)
                };
                pairs.push((d, read(x, names, ctx, budget, depth + 1)?));
            }
            if kind == "record" {
                Value::record(
                    pairs
                        .into_iter()
                        .map(|(d, x)| match d {
                            Value::Str(a) => Ok((a, x)),
                            _ => Err("record field must be string".into()),
                        })
                        .collect::<Result<Vec<_>, String>>()?,
                    ctx,
                )
                .map_err(|e| e.message().to_owned())?
            } else {
                Value::fcn(pairs, ctx).map_err(|e| e.message().to_owned())?
            }
        }
        "set" | "raw-set" => {
            keys(
                j,
                if kind == "set" {
                    &["repr", "order", "items", "witness"]
                } else {
                    &["repr", "order", "items", "normalized"]
                },
            )?;
            checked_order(j, ctx)?;
            let values = items(j)?
                .iter()
                .map(|x| read(x, names, ctx, budget, depth + 1))
                .collect::<Result<Vec<_>, _>>()?;
            if kind == "raw-set" {
                let normalized = j["normalized"]
                    .as_bool()
                    .ok_or("raw set normalized must be Boolean")?;
                let v = Value::set_raw(values, ctx);
                if normalized && let Value::Set(s) = &v {
                    s.elements(ctx).map_err(|e| e.message().to_owned())?;
                }
                v
            } else {
                let indices = j["witness"]
                    .as_array()
                    .ok_or("set witness must be an index permutation")?;
                if indices.len() != values.len() {
                    return Err("set witness has wrong length".into());
                }
                let mut seen = vec![false; values.len()];
                let mut construction = Vec::with_capacity(values.len());
                for index in indices {
                    let i = index
                        .as_u64()
                        .and_then(|i| usize::try_from(i).ok())
                        .ok_or("set witness index must be a nonnegative count")?;
                    if i >= values.len() || seen[i] {
                        return Err("set witness has duplicate or out-of-range index".into());
                    }
                    seen[i] = true;
                    construction.push(values[i].clone());
                }
                let v = Value::set(construction, ctx).map_err(|e| e.message().to_owned())?;
                if !exact_items(&v, &values, ctx)? {
                    return Err(
                        "set witness does not reproduce exact stored order/representation".into(),
                    );
                }
                v
            }
        }
        _ => return Err(format!("unsupported stored representation `{kind}`")),
    };
    // Reconstruct expected shape without any caller normalization witness; exact
    // comparison checks literal/domain/order claims as well as concrete children.
    let mut expected = j.clone();
    remove_witnesses(&mut expected);
    if kind == "set" {
        expected
            .as_object_mut()
            .ok_or("stored set must be object")?
            .remove("witness");
    }
    if shape(&value, ctx)? != expected {
        return Err("stored representation disagrees with checked constructor".into());
    }
    if let Value::Fcn(f) = &value
        && kind == "fcn"
        && f.kind() != FcnKind::General
    {
        return Err("general function domain has tuple/record shape".into());
    }
    Ok(value)
}

fn remove_witnesses(j: &mut J) {
    match j {
        J::Object(o) => {
            if o.get("repr").and_then(J::as_str) == Some("set") {
                o.remove("witness");
            }
            for v in o.values_mut() {
                remove_witnesses(v);
            }
        }
        J::Array(a) => {
            for v in a {
                remove_witnesses(v);
            }
        }
        _ => {}
    }
}

/// Checks the complete opt-in document before cloning or constructing its values.
/// The 4 MiB/16,384-node conservative JSON budget is shared across the word.
/// Metadata is included; input JSON parsing/allocation is outside this admission.
///
/// # Errors
/// Document byte/node/nesting limits.
pub(crate) fn document_admission(j: &J) -> Result<(), String> {
    preflight(j, &mut Budget::default(), 0)
}

/// Checks already parsed states at the public typed-document boundary.
/// Uses the same whole-word budget without cloning supplied maps.
pub(crate) fn states_admission(states: &[serde_json::Map<String, J>]) -> Result<(), String> {
    let mut budget = Budget::default();
    for state in states {
        budget.node(0)?;
        for (name, value) in state {
            budget.text(name)?;
            preflight(value, &mut budget, 0)?;
        }
    }
    Ok(())
}

pub(crate) fn word_admission(doc: &crate::TraceDoc) -> Result<(), String> {
    members(doc.states.len())?;
    members(doc.variables.len())?;
    members(doc.properties.len())?;
    if doc.labels.len() != doc.states.len() {
        return Err("stored trace labels/state lengths disagree".into());
    }
    let mut budget = Budget::default();
    for name in doc.variables.iter().chain(&doc.properties) {
        budget.node(0)?;
        budget.text(name)?;
    }
    for label in doc.labels.iter().flatten() {
        budget.node(0)?;
        budget.text(&label.name)?;
        if let Some(location) = &label.location {
            budget.text(location)?;
        }
    }
    if let Some(
        crate::Ending::Invariant(name)
        | crate::Ending::Property(name)
        | crate::Ending::Error { ec: Some(name) },
    ) = &doc.ending
    {
        budget.text(name)?;
    }
    for state in &doc.states {
        members(state.len())?;
        budget.node(0)?;
        for (name, value) in state {
            budget.text(name)?;
            preflight(value, &mut budget, 0)?;
        }
    }
    Ok(())
}
