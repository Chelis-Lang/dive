//! Reading TLC model configuration (`.cfg`) files.

use crate::ApiError;
use dive_config::{
    ConfigParse, ConfigValue, ConstantEntry, Diagnostic, ModelConfig, Name, Severity, Spanned,
};
use dive_syntax::diagnostics::line_col;
use serde_json::{Map, Value, json};
use std::path::Path;

/// The outcome of reading one cfg file.
#[derive(Debug, Clone)]
pub struct ConfigReport {
    file: String,
    source: String,
    parse: ConfigParse,
}

/// Reads cfg text. `file` names it in reports.
#[must_use]
pub fn parse_config_source(file: &str, source: &str) -> ConfigReport {
    ConfigReport {
        file: file.to_owned(),
        source: source.to_owned(),
        parse: dive_config::parse(source),
    }
}

/// Reads and parses a cfg file, as TLC's `-config` does.
///
/// Malformed UTF-8 is read as U+FFFD, like TLC's reader, with a
/// `DIVE-C0004` warning.
///
/// # Errors
///
/// [`ApiError::Io`] if the file cannot be read, and
/// [`ApiError::Unsupported`] for a configuration embedded in a `.tla` file
/// (TLC reads those through `MonolithSpecExtractor`; dive does not yet).
pub fn parse_config_file(path: &Path) -> Result<ConfigReport, ApiError> {
    let shown = path.display().to_string();
    if path.extension().is_some_and(|e| e == "tla") {
        return Err(ApiError::Unsupported {
            feature: "configurations embedded in a .tla file (MonolithSpecExtractor)".to_owned(),
        });
    }
    let bytes = std::fs::read(path).map_err(|error| ApiError::Io {
        path: shown.clone(),
        error,
    })?;
    let (source, parse) = dive_config::parse_bytes(&bytes);
    Ok(ConfigReport {
        file: shown,
        source,
        parse,
    })
}

impl ConfigReport {
    /// Whether TLC's cfg reader accepts the file.
    #[must_use]
    pub fn accepted(&self) -> bool {
        self.parse.accepted()
    }

    /// The parse result.
    #[must_use]
    pub fn parse(&self) -> &ConfigParse {
        &self.parse
    }

    pub(crate) fn source_bytes(&self) -> &[u8] {
        self.source.as_bytes()
    }

    /// The configuration, if accepted.
    #[must_use]
    pub fn config(&self) -> Option<&ModelConfig> {
        self.parse.config()
    }

    /// Diagnostics, one per line, like `file:3:7: error[DIVE-C0013]: message`,
    /// with TLC's equivalent report after each error.
    #[must_use]
    pub fn render_human(&self) -> String {
        use std::fmt::Write as _;
        let mut s = String::new();
        for d in self.parse.diagnostics() {
            let (line, col) = line_col(&self.source, d.range.start());
            let sev = match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            };
            // Writing to a String cannot fail.
            let _ = writeln!(
                s,
                "{}:{line}:{col}: {sev}[{}]: {}",
                self.file, d.code, d.message
            );
            if let Some(h) = &d.help {
                let _ = writeln!(s, "  help: {h}");
            }
            if let Some(t) = d.tlc {
                let _ = writeln!(
                    s,
                    "  tlc: {} ({}), exit {}{}",
                    t.name,
                    t.code,
                    t.exit_code,
                    t.line.map(|l| format!(", line {l}")).unwrap_or_default()
                );
            }
        }
        s
    }

    /// The report as JSON (the `config` object of `dive parse --output json`).
    ///
    /// ```text
    /// { "file", "accepted",
    ///   "diagnostics": [ { "code", "severity", "message", "help", "span",
    ///                      "tlc": { "code", "name", "exit_code", "line", "detail" } | null } ],
    ///   "model": { "init", "next", "specification", "view", "symmetry", "alias",
    ///              "periodic", "rl_reward",            // name objects or null
    ///              "check_deadlock": bool | null, "checks_deadlock": bool,
    ///              "invariants", "properties", "constraints", "action_constraints",
    ///              "postconditions", "possible",       // arrays of name objects
    ///              "constants": [ { "kind": "assignment", "name", "module", "args", "value" }
    ///                           | { "kind": "substitution", "name", "module", "target" } ],
    ///              "model_values": [ "..." ] } | null }
    /// ```
    ///
    /// A name object is `{ "text", "span" }`; a value is `{ "int" }`,
    /// `{ "string" }`, `{ "bool" }`, `{ "model_value" }` or `{ "set": [...] }`,
    /// each with a `span`.
    /// This is a nested configuration fragment. Its wire contract inherits
    /// the enclosing `parse` or `check` report's version; it is not an
    /// independently versioned operation envelope.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let diagnostics: Vec<Value> = self
            .parse
            .diagnostics()
            .iter()
            .map(|d| self.diagnostic_json(d))
            .collect();
        json!({
            "file": self.file,
            "accepted": self.accepted(),
            "diagnostics": diagnostics,
            "model": self.parse.config().map(|c| self.model_json(c)),
        })
    }

    fn span(&self, r: dive_config::TextRange) -> Value {
        let (line, column) = line_col(&self.source, r.start());
        let (end_line, end_column) = line_col(&self.source, r.end());
        json!({
            "start": u32::from(r.start()),
            "end": u32::from(r.end()),
            "line": line,
            "column": column,
            "end_line": end_line,
            "end_column": end_column,
        })
    }

    fn diagnostic_json(&self, d: &Diagnostic) -> Value {
        json!({
            "code": d.code.as_str(),
            "severity": match d.severity {
                Severity::Error => "error",
                Severity::Warning => "warning",
            },
            "message": d.message,
            "help": d.help,
            "span": self.span(d.range),
            "tlc": d.tlc.map(|t| json!({
                "code": t.code,
                "name": t.name,
                "exit_code": t.exit_code,
                "line": t.line,
                "detail": t.detail,
            })),
        })
    }

    fn name_json(&self, n: &Name) -> Value {
        json!({ "text": n.text, "span": self.span(n.span) })
    }

    fn value_json(&self, v: &Spanned<ConfigValue>) -> Value {
        let mut m = Map::new();
        match &v.value {
            ConfigValue::Int(i) => m.insert("int".to_owned(), json!(i)),
            ConfigValue::String(s) => m.insert("string".to_owned(), json!(s)),
            ConfigValue::Bool(b) => m.insert("bool".to_owned(), json!(b)),
            ConfigValue::ModelValue(s) => m.insert("model_value".to_owned(), json!(s)),
            ConfigValue::Set(items) => m.insert(
                "set".to_owned(),
                Value::Array(items.iter().map(|i| self.value_json(i)).collect()),
            ),
        };
        m.insert("span".to_owned(), self.span(v.span));
        Value::Object(m)
    }

    fn model_json(&self, c: &ModelConfig) -> Value {
        let one = |n: &Option<Name>| n.as_ref().map(|n| self.name_json(n));
        let list = |l: &[Name]| l.iter().map(|n| self.name_json(n)).collect::<Vec<_>>();
        let constants: Vec<Value> = c
            .constants
            .iter()
            .map(|e| match e {
                ConstantEntry::Assignment {
                    name,
                    args,
                    module,
                    value,
                } => json!({
                    "kind": "assignment",
                    "name": self.name_json(name),
                    "module": module.as_ref().map(|m| self.name_json(m)),
                    "args": args.iter().map(|a| self.value_json(a)).collect::<Vec<_>>(),
                    "value": self.value_json(value),
                }),
                ConstantEntry::Substitution {
                    name,
                    module,
                    target,
                } => json!({
                    "kind": "substitution",
                    "name": self.name_json(name),
                    "module": module.as_ref().map(|m| self.name_json(m)),
                    "target": self.name_json(target),
                }),
            })
            .collect();
        json!({
            "init": one(&c.init),
            "next": one(&c.next),
            "specification": one(&c.specification),
            "view": one(&c.view),
            "symmetry": one(&c.symmetry),
            "alias": one(&c.alias),
            "periodic": one(&c.periodic),
            "rl_reward": one(&c.rl_reward),
            "check_deadlock": c.check_deadlock.as_ref().map(|d| d.value),
            "checks_deadlock": c.checks_deadlock(),
            "invariants": list(&c.invariants),
            "properties": list(&c.properties),
            "constraints": list(&c.constraints),
            "action_constraints": list(&c.action_constraints),
            "postconditions": list(&c.postconditions),
            "possible": list(&c.possible),
            "constants": constants,
            "model_values": c.model_values,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepted_config_json() {
        let r = parse_config_source(
            "M.cfg",
            "CONSTANT N = 3 S = {a, b}\nT <- [Mod] U\nINIT Init\nNEXT Next\nINVARIANT Inv\n",
        );
        assert!(r.accepted());
        let v = r.to_json();
        assert_eq!(v["model"]["init"]["text"], "Init");
        assert_eq!(v["model"]["invariants"][0]["text"], "Inv");
        assert_eq!(v["model"]["checks_deadlock"], true);
        assert_eq!(
            v["model"]["constants"][1]["value"]["set"][1]["model_value"],
            "b"
        );
        assert_eq!(v["model"]["constants"][2]["module"]["text"], "Mod");
        assert_eq!(v["model"]["model_values"], json!(["a", "b"]));
    }

    #[test]
    fn rejected_config_reports_tlc_equivalent() {
        let r = parse_config_source("M.cfg", "INIT Init\nINVARIANT Inv ;\n");
        assert!(!r.accepted());
        let v = r.to_json();
        assert_eq!(v["model"], Value::Null);
        assert_eq!(v["diagnostics"][0]["code"], "DIVE-C0001");
        assert_eq!(v["diagnostics"][0]["tlc"]["code"], 5007);
        assert_eq!(v["diagnostics"][0]["tlc"]["exit_code"], 151);
        assert_eq!(v["diagnostics"][0]["span"]["line"], 2);
        assert!(
            r.render_human()
                .contains("CFG_LEXICAL_ERROR (5007), exit 151, line 2")
        );
    }

    #[test]
    fn tla_configs_are_unsupported() {
        let e = parse_config_file(Path::new("Spec.tla"));
        assert!(matches!(e, Err(ApiError::Unsupported { .. })));
    }
}
