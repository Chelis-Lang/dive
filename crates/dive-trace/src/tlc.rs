//! Reading TLC's `-dumpTrace json` values against a model.
//!
//! TLC's JSON form ([`dive_value::json`]) loses information:
//! - a finite set and a sequence are both arrays;
//! - a string and a model value are both strings;
//! - a function's domain elements are object keys in TLC's string form, so
//!   the key `"1"` may be the integer 1 or the string "1".
//!
//! TLC cannot read these documents back either. dive reads one by
//! **interpretation**: every place where the JSON is ambiguous is a
//! *decision*, made once per path in the value (variable, record field,
//! array element, ...) for the whole trace, so a variable has one type
//! throughout. Some decisions are forced by the JSON itself: an array with
//! repeated elements, or with integers out of ascending order, or with
//! elements of different JSON kinds, is not a set TLC wrote. A string is a
//! model value only if the configuration declares a model value of that
//! name. The rest are searched: the validator runs on each interpretation
//! (default first, then improving single changes, then, when there are few
//! enough, all of them), and
//! - a trace is **valid** if some interpretation of it is a valid trace;
//! - it is **invalid** only if the search was exhaustive and no
//!   interpretation is valid (the failure reported is the default
//!   interpretation's);
//! - otherwise the result is **unsupported**, naming the ambiguity.
//!
//! So a valid result is always witnessed by an exact trace
//! ([`TlcReading::trace`]), which the validator checked like any other.
//!
//! **`ALIAS`.** With an `ALIAS`, TLC writes the alias's record instead of
//! each state. If every variable is a field of the record, the state is
//! read from those fields (the alias expression is not evaluated, since it
//! may run commands, as `IOExec` does); otherwise the trace is
//! unsupported.

use std::collections::{BTreeMap, BTreeSet, HashSet};

use dive_eval::{Scope, St};
use dive_ir::{Effects, Expr, ExprId, JunctionKind, OpRef};
use dive_model::Model;
use dive_value::{Ctx, Interner, Value};
use serde_json::{Map, Value as J};

use crate::format::{Ending, TraceDoc};
use crate::trace::{Trace, decode_state, variables};
use crate::validate::{Check, Failure, Replay, Report, Run, Validation};

/// The most interpretations tried exhaustively (`2^10`).
const EXHAUSTIVE_LIMIT: u32 = 10;

/// The result of reading and validating a TLC document.
#[derive(Clone, Debug)]
pub struct TlcReading {
    /// The verdict and notes.
    pub report: Report,
    /// The exact trace the verdict is about: the witness for a valid
    /// result, the default interpretation otherwise (if it reads at all).
    pub trace: Option<Trace>,
}

/// A place where TLC's JSON is ambiguous.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
enum Kind {
    /// An array: a sequence (`true`) or a set (`false`).
    Array,
    /// A string: a model value (`true`) or a string (`false`).
    Str,
    /// An object's keys: TLC value text (`true`) or plain strings (`false`).
    Keys,
}

type Decision = (Kind, String);

/// The reading tried first: a set (TLC writes sets in normalized order, and
/// an array that is not is already a sequence), a model value when the
/// configuration declares one of that name, and keys as TLC values.
fn preferred(kind: Kind) -> bool {
    kind != Kind::Array
}

/// Fixed choices and the free decisions of a document.
#[derive(Default)]
struct Analysis {
    forced: BTreeMap<Decision, bool>,
    seen: BTreeSet<Decision>,
}

impl Analysis {
    fn free(&self) -> Vec<Decision> {
        self.seen
            .iter()
            .filter(|d| !self.forced.contains_key(*d))
            .cloned()
            .collect()
    }
}

/// How each decision is made.
struct Reader<'a> {
    names: &'a Interner,
    ctx: Ctx,
    model_values: &'a [String],
    choice: BTreeMap<Decision, bool>,
}

impl Reader<'_> {
    fn pick(&self, kind: Kind, path: &str) -> bool {
        self.choice
            .get(&(kind, path.to_owned()))
            .copied()
            .unwrap_or_else(|| preferred(kind))
    }

    fn read(&self, j: &J, path: &str) -> Result<Value, String> {
        match j {
            J::Bool(b) => Ok(Value::Bool(*b)),
            J::Number(n) => n
                .as_i64()
                .map(Value::Int)
                .ok_or_else(|| format!("`{n}` is not an integer TLC writes")),
            J::String(s) => Ok(
                if self.model_values.contains(s) && self.pick(Kind::Str, path) {
                    Value::model(self.names, s)
                } else {
                    Value::string(self.names, s)
                },
            ),
            J::Null => Err("TLC never writes null".to_owned()),
            J::Array(a) => {
                let sub = format!("{path}[]");
                let elems = a
                    .iter()
                    .map(|e| self.read(e, &sub))
                    .collect::<Result<Vec<_>, _>>()?;
                if self.pick(Kind::Array, path) {
                    Ok(Value::tuple(elems))
                } else {
                    let count = elems.len();
                    let set = Value::set(elems, self.ctx).map_err(|e| e.message().to_owned())?;
                    // TLC writes a set's elements once each.
                    match &set {
                        Value::Set(s) if s.size(self.ctx).ok() == Some(count as u64) => Ok(set),
                        _ => Err("repeated set elements".to_owned()),
                    }
                }
            }
            J::Object(m) => {
                let parsed = self.pick(Kind::Keys, path);
                let mut pairs = Vec::with_capacity(m.len());
                for (k, x) in m {
                    let key = if parsed {
                        parse_key(k, self.names, self.ctx, self.model_values)
                    } else {
                        None
                    };
                    let key = key.unwrap_or_else(|| Value::string(self.names, k));
                    pairs.push((key, self.read(x, &key_path(path, k, self.model_values))?));
                }
                if pairs.iter().all(|(k, _)| matches!(k, Value::Str(_))) {
                    let fields = pairs
                        .into_iter()
                        .filter_map(|(k, v)| match k {
                            Value::Str(a) => Some((a, v)),
                            _ => None,
                        })
                        .collect();
                    Value::record(fields, self.ctx).map_err(|e| e.message().to_owned())
                } else {
                    Value::fcn(pairs, self.ctx).map_err(|e| e.message().to_owned())
                }
            }
        }
    }

    fn state(&self, vars: &[String], s: &Map<String, J>) -> Result<crate::trace::State, String> {
        decode_state(vars, s, |name, j| self.read(j, name))
    }
}

/// The path of an object's member: by field name for record-like keys, one
/// shared path for keys that are TLC values (a function's domain).
fn key_path(path: &str, key: &str, model_values: &[String]) -> String {
    if is_plain_key(key, model_values) {
        format!("{path}.{key}")
    } else {
        format!("{path}.*")
    }
}

fn is_plain_key(key: &str, model_values: &[String]) -> bool {
    let mut cs = key.chars();
    cs.next()
        .is_some_and(|c| c.is_ascii_alphabetic() || c == '_')
        && key.chars().all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !model_values.iter().any(|m| m == key)
        && key != "TRUE"
        && key != "FALSE"
}

fn analyze(states: &[Map<String, J>], model_values: &[String]) -> Analysis {
    let mut a = Analysis::default();
    for s in states {
        for (name, j) in s {
            walk(&mut a, j, name, model_values);
        }
    }
    a
}

fn walk(a: &mut Analysis, j: &J, path: &str, model_values: &[String]) {
    match j {
        J::String(s) if model_values.contains(s) => {
            a.seen.insert((Kind::Str, path.to_owned()));
        }
        J::Array(items) => {
            let d = (Kind::Array, path.to_owned());
            a.seen.insert(d.clone());
            let repeated = items
                .iter()
                .enumerate()
                .any(|(i, x)| items[..i].contains(x));
            let ints: Option<Vec<i64>> = items.iter().map(J::as_i64).collect();
            let unsorted = ints.is_some_and(|v| v.windows(2).any(|w| w[0] >= w[1]));
            let mixed = items
                .windows(2)
                .any(|w| std::mem::discriminant(&w[0]) != std::mem::discriminant(&w[1]));
            if repeated || unsorted || mixed {
                a.forced.insert(d, true);
            }
            let sub = format!("{path}[]");
            for x in items {
                walk(a, x, &sub, model_values);
            }
        }
        J::Object(m) => {
            if m.keys().any(|k| !is_plain_key(k, model_values)) {
                a.seen.insert((Kind::Keys, path.to_owned()));
            }
            for (k, x) in m {
                walk(a, x, &key_path(path, k, model_values), model_values);
            }
        }
        _ => {}
    }
}

/// Parses a function-domain key written in TLC's value syntax: integers,
/// `TRUE`/`FALSE`, quoted strings, declared model values, `<<...>>`,
/// `{...}` and `[f |-> v, ...]`. `None` for anything else, which is read as
/// a plain string.
fn parse_key(text: &str, names: &Interner, ctx: Ctx, model_values: &[String]) -> Option<Value> {
    let mut p = KeyParser {
        s: text.as_bytes(),
        i: 0,
        names,
        ctx,
        model_values,
    };
    let v = p.value()?;
    p.ws();
    (p.i == p.s.len()).then_some(v)
}

struct KeyParser<'a> {
    s: &'a [u8],
    i: usize,
    names: &'a Interner,
    ctx: Ctx,
    model_values: &'a [String],
}

impl KeyParser<'_> {
    fn ws(&mut self) {
        while self.s.get(self.i).is_some_and(u8::is_ascii_whitespace) {
            self.i += 1;
        }
    }

    fn eat(&mut self, t: &str) -> bool {
        self.ws();
        if self.s[self.i..].starts_with(t.as_bytes()) {
            self.i += t.len();
            true
        } else {
            false
        }
    }

    fn list(&mut self, close: &str) -> Option<Vec<Value>> {
        let mut out = Vec::new();
        if self.eat(close) {
            return Some(out);
        }
        loop {
            out.push(self.value()?);
            if self.eat(close) {
                return Some(out);
            }
            if !self.eat(",") {
                return None;
            }
        }
    }

    fn value(&mut self) -> Option<Value> {
        self.ws();
        if self.eat("<<") {
            return Some(Value::tuple(self.list(">>")?));
        }
        if self.eat("{") {
            return Value::set(self.list("}")?, self.ctx).ok();
        }
        if self.eat("[") {
            let mut fields = Vec::new();
            loop {
                let name = self.ident()?;
                if !self.eat("|->") {
                    return None;
                }
                fields.push((self.names.intern(&name), self.value()?));
                if self.eat("]") {
                    return Value::record(fields, self.ctx).ok();
                }
                if !self.eat(",") {
                    return None;
                }
            }
        }
        if self.eat("\"") {
            let mut out = String::new();
            loop {
                match *self.s.get(self.i)? {
                    b'"' => {
                        self.i += 1;
                        return Some(Value::string(self.names, &out));
                    }
                    b'\\' => {
                        let c = *self.s.get(self.i + 1)?;
                        out.push(match c {
                            b'n' => '\n',
                            b't' => '\t',
                            b'r' => '\r',
                            b'f' => '\u{c}',
                            other => char::from(other),
                        });
                        self.i += 2;
                    }
                    _ => {
                        let rest = std::str::from_utf8(&self.s[self.i..]).ok()?;
                        let c = rest.chars().next()?;
                        out.push(c);
                        self.i += c.len_utf8();
                    }
                }
            }
        }
        let start = self.i;
        if self.s.get(self.i) == Some(&b'-') {
            self.i += 1;
        }
        let digits = self.i;
        while self.s.get(self.i).is_some_and(u8::is_ascii_digit) {
            self.i += 1;
        }
        if self.i > digits {
            let text = std::str::from_utf8(&self.s[start..self.i]).ok()?;
            return text.parse::<i64>().ok().map(Value::Int);
        }
        self.i = start;
        let id = self.ident()?;
        match id.as_str() {
            "TRUE" => Some(Value::Bool(true)),
            "FALSE" => Some(Value::Bool(false)),
            _ if self.model_values.contains(&id) => Some(Value::model(self.names, &id)),
            _ => None,
        }
    }

    fn ident(&mut self) -> Option<String> {
        self.ws();
        let start = self.i;
        while self
            .s
            .get(self.i)
            .is_some_and(|c| c.is_ascii_alphanumeric() || *c == b'_')
        {
            self.i += 1;
        }
        (self.i > start).then(|| String::from_utf8_lossy(&self.s[start..self.i]).into_owned())
    }
}

/// The states as variable assignments: the document's own, or recovered
/// from `ALIAS` records.
fn variable_states(
    model: &Model,
    doc: &TraceDoc,
    notes: &mut Vec<String>,
) -> Result<Vec<Map<String, J>>, String> {
    let vars = variables(model);
    let exact = doc
        .states
        .iter()
        .all(|s| s.len() == vars.len() && vars.iter().all(|v| s.contains_key(v)));
    if exact || model.alias.is_none() {
        return Ok(doc.states.clone());
    }
    if doc
        .states
        .iter()
        .all(|s| vars.iter().all(|v| s.contains_key(v)))
    {
        notes.push("TLC wrote ALIAS records: the states were read from their fields named after the variables (the ALIAS expression was not evaluated)".to_owned());
        return Ok(doc
            .states
            .iter()
            .map(|s| {
                vars.iter()
                    .filter_map(|v| s.get(v).map(|j| (v.clone(), j.clone())))
                    .collect()
            })
            .collect());
    }
    Err("ALIAS: TLC wrote the alias's records, which do not contain every variable, so the states cannot be recovered".to_owned())
}

/// Reads a TLC `-dumpTrace json` document against `model` and validates
/// it (see the module documentation). `model_values` are the
/// configuration's model value names (`dive_config::ModelConfig::model_values`).
/// The document's ending must be set ([`TraceDoc::with_ending`]).
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one search, read top to bottom: default, hill-climb, exhaustive, verdict"
)]
pub fn validate_tlc(
    model: &Model,
    doc: &TraceDoc,
    model_values: &[String],
    _replay: Option<&Replay>,
) -> TlcReading {
    let length = doc.states.len();
    let report = |validation, notes| Report {
        validation,
        notes,
        length,
    };
    // Validate public metadata before the interpretation heuristic indexes
    // expressions or evaluates a predicate, just as native replay does.
    if let Err(error) = model.validate_context() {
        return TlcReading {
            report: report(Validation::Unsupported(error.to_string()), Vec::new()),
            trace: None,
        };
    }
    if model.has_runtime_names() {
        return TlcReading {
            report: report(
                Validation::Unsupported(crate::RUNTIME_NAMES_UNSUPPORTED.to_owned()),
                Vec::new(),
            ),
            trace: None,
        };
    }
    let Some(ending) = doc.ending.clone() else {
        return TlcReading {
            report: report(
                Validation::Invalid(Failure {
                    state: 0,
                    check: Check::Shape,
                    detail: "TLC's document does not say how the trace ends; give the claim"
                        .to_owned(),
                }),
                Vec::new(),
            ),
            trace: None,
        };
    };
    if !doc.properties.is_empty() {
        // The interpretation heuristic scores initial predicates. Preflight
        // before that evaluation too, so a refusal cannot execute effects.
        let preflight = Run::new(model).prepare_periodic(&ending, &doc.properties);
        if let Err(stop) = preflight {
            return TlcReading {
                report: report(stop.into(), Vec::new()),
                trace: None,
            };
        }
    }
    let mut notes = Vec::new();
    let states = match variable_states(model, doc, &mut notes) {
        Ok(s) => s,
        Err(why) => {
            return TlcReading {
                report: report(Validation::Unsupported(why), notes),
                trace: None,
            };
        }
    };
    let analysis = analyze(&states, model_values);
    let free = analysis.free();
    let vars = variables(model);
    let mut search = Search {
        model,
        vars: &vars,
        states: &states,
        ending: &ending,
        labels: &doc.labels,
        properties: &doc.properties,
        length: doc.length,
        model_values,
        forced: &analysis.forced,
        free: &free,
        conjuncts: init_conjuncts(model),
        tried: HashSet::new(),
    };
    let default = vec![false; free.len()];
    let first = search.evaluate(&default);
    let mut best = first.clone();
    if !best.valid() {
        // Hill-climb: take the best single change while it improves.
        let mut cur = default.clone();
        loop {
            let mut step: Option<(Vec<bool>, Outcome)> = None;
            for i in 0..free.len() {
                let mut c = cur.clone();
                c[i] = !c[i];
                if search.tried.contains(&c) {
                    continue;
                }
                let o = search.evaluate(&c);
                if step.as_ref().is_none_or(|(_, s)| o.rank() > s.rank()) {
                    step = Some((c, o));
                }
            }
            match step {
                Some((c, o)) if o.rank() > best.rank() => {
                    cur = c;
                    best = o;
                    if best.valid() {
                        break;
                    }
                }
                _ => break,
            }
        }
    }
    let exhaustive = free.len() <= EXHAUSTIVE_LIMIT as usize;
    if !best.valid() && exhaustive {
        for bits in 0u32..(1u32 << free.len()) {
            let c: Vec<bool> = (0..free.len()).map(|i| bits & (1 << i) != 0).collect();
            if search.tried.contains(&c) {
                continue;
            }
            let o = search.evaluate(&c);
            if o.rank() > best.rank() {
                best = o;
                if best.valid() {
                    break;
                }
            }
        }
    }
    let tried = search.tried.len();
    if !free.is_empty() {
        notes.push(format!(
            "TLC's JSON is ambiguous in {} place(s) ({}); {tried} interpretation(s) tried",
            free.len(),
            free.iter()
                .map(|(k, p)| format!(
                    "{} at `{p}`",
                    match k {
                        Kind::Array => "set or sequence",
                        Kind::Str => "string or model value",
                        Kind::Keys => "function domain",
                    }
                ))
                .collect::<Vec<_>>()
                .join(", ")
        ));
    }
    let (validation, trace) = if best.valid() {
        (Validation::Valid, best.trace)
    } else if best.unsupported() {
        (best.validation, best.trace)
    } else if free.is_empty() || exhaustive && tried == 1usize << free.len() {
        if !free.is_empty() {
            notes.push(
                "no interpretation validates; the failure shown is the default interpretation's"
                    .to_owned(),
            );
        }
        (first.validation, first.trace)
    } else {
        (
            Validation::Unsupported(format!(
                "TLC's JSON is too ambiguous to search exhaustively ({} decisions) and no interpretation tried validates; the default interpretation's failure: {}",
                free.len(),
                match &first.validation {
                    Validation::Invalid(f) =>
                        format!("state {}, {}: {}", f.state, f.check.name(), f.detail),
                    other => other.kind().to_owned(),
                }
            )),
            first.trace,
        )
    };
    notes.extend(best.notes);
    notes.dedup();
    // An interpretation tried after the verdict's may have created a name.
    TlcReading {
        report: crate::validate::guard_runtime_names(model, report(validation, notes)),
        trace,
    }
}

#[derive(Clone, Debug)]
struct Outcome {
    validation: Validation,
    score: (u64, u64),
    trace: Option<Trace>,
    notes: Vec<String>,
}

impl Outcome {
    fn valid(&self) -> bool {
        self.validation.is_valid()
    }

    fn unsupported(&self) -> bool {
        matches!(self.validation, Validation::Unsupported(_))
    }

    /// Valid beats unsupported beats invalid; then the further, the better.
    fn rank(&self) -> (u8, (u64, u64)) {
        let class = match self.validation {
            Validation::Valid => 2,
            Validation::Unsupported(_) => 1,
            Validation::Invalid(_) => 0,
        };
        (class, self.score)
    }
}

struct Search<'a> {
    model: &'a Model,
    vars: &'a [String],
    states: &'a [Map<String, J>],
    ending: &'a Ending,
    labels: &'a [Option<crate::format::Label>],
    properties: &'a [String],
    length: Option<usize>,
    model_values: &'a [String],
    forced: &'a BTreeMap<Decision, bool>,
    free: &'a [Decision],
    conjuncts: Vec<(Scope, ExprId)>,
    tried: HashSet<Vec<bool>>,
}

impl Search<'_> {
    fn evaluate(&mut self, choice: &[bool]) -> Outcome {
        self.tried.insert(choice.to_vec());
        let mut map = self.forced.clone();
        // `false` is each decision's preferred reading.
        for (d, c) in self.free.iter().zip(choice) {
            map.insert(d.clone(), *c != preferred(d.0));
        }
        let reader = Reader {
            names: &self.model.names,
            ctx: self.model.evaluator.options().ctx,
            model_values: self.model_values,
            choice: map,
        };
        let mut states = Vec::with_capacity(self.states.len());
        for (k, s) in self.states.iter().enumerate() {
            match reader.state(self.vars, s) {
                Ok(st) => states.push(st),
                Err(detail) => {
                    return Outcome {
                        validation: Validation::Invalid(Failure {
                            state: k,
                            check: Check::Shape,
                            detail,
                        }),
                        score: (0, 0),
                        trace: None,
                        notes: Vec::new(),
                    };
                }
            }
        }
        let mut run = Run::new(self.model);
        let init_true = states.first().map_or(0, |s0| {
            let st = St::state(s0.clone());
            self.conjuncts
                .iter()
                .filter(|(scope, e)| {
                    // Scoring is only a heuristic: it must not write a
                    // register, consume a draw or print before replay checks
                    // Init in its actual short-circuit order.
                    !self.model.ir.node(*e).effects.intersects(Effects::IMPURE)
                        && !self.model.evaluator.scope_shares_side_effects(scope)
                        && matches!(
                            self.model.evaluator.eval_scoped(scope, *e, &st),
                            Ok(Value::Bool(true))
                        )
                })
                .count() as u64
        });
        let trace = Trace {
            states,
            labels: self.labels.to_vec(),
            ending: self.ending.clone(),
            length: self.length,
            properties: self.properties.to_vec(),
        };
        let validation = run.validate(&trace);
        Outcome {
            validation,
            score: (init_true, run.passed),
            trace: Some(trace),
            notes: std::mem::take(&mut run.notes).into_iter().collect(),
        }
    }
}

/// The conjuncts of the initial predicates, through definitions without
/// parameters: each is evaluated alone to score an interpretation.
fn init_conjuncts(model: &Model) -> Vec<(Scope, ExprId)> {
    let mut out = Vec::new();
    for init in &model.init {
        split(model, &init.pred.scope, init.pred.expr, &mut out, 0);
    }
    out
}

fn split(model: &Model, scope: &Scope, e: ExprId, out: &mut Vec<(Scope, ExprId)>, depth: u32) {
    let ev = &model.evaluator;
    if depth < 32 {
        match model.ir.expr(e) {
            Expr::Junction {
                kind: JunctionKind::And,
                items,
                ..
            } => {
                for &i in items {
                    split(model, scope, i, out, depth + 1);
                }
                return;
            }
            Expr::Def(_)
            | Expr::Apply {
                callee: OpRef::Def(_),
                ..
            } => {
                if let Ok((inner, d)) = ev.scope_call(scope, e)
                    && !ev.is_value_override(d)
                {
                    split(model, &inner, model.ir.def(d).body, out, depth + 1);
                    return;
                }
            }
            _ => {}
        }
    }
    out.push((scope.clone(), e));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_tlc_keys() {
        let names = Interner::new();
        let ctx = Ctx::tlc();
        let mv = vec!["n1".to_owned()];
        assert_eq!(parse_key("1", &names, ctx, &mv), Some(Value::Int(1)));
        assert_eq!(parse_key("-12", &names, ctx, &mv), Some(Value::Int(-12)));
        assert_eq!(
            parse_key("n1", &names, ctx, &mv),
            Some(Value::model(&names, "n1"))
        );
        assert_eq!(parse_key("n2", &names, ctx, &mv), None);
        assert_eq!(
            parse_key("<<1, \"a\">>", &names, ctx, &mv),
            Some(Value::tuple(vec![
                Value::Int(1),
                Value::string(&names, "a")
            ]))
        );
        assert!(parse_key("{1, 2}", &names, ctx, &mv).is_some());
        assert!(parse_key("[a |-> 1]", &names, ctx, &mv).is_some());
        assert_eq!(parse_key("1 2", &names, ctx, &mv), None);
    }

    #[test]
    fn forces_what_the_json_decides() {
        let states: Vec<Map<String, J>> = serde_json::from_str(
            r#"[{"a": [3, 1], "b": [1, 1], "c": [1, 2], "d": [], "e": [1, "x"], "f": {"1": 2}}]"#,
        )
        .unwrap_or_default();
        let a = analyze(&states, &[]);
        let free: Vec<String> = a.free().into_iter().map(|(_, p)| p).collect();
        assert_eq!(free, ["c", "d", "f"]);
    }
}
