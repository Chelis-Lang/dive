//! Counterexample transports: legacy version 1, opt-in stored version 2, and TLC JSON.
//!
//! # dive's format
//!
//! ```json
//! {
//!   "format": "dive-trace",
//!   "version": 1,
//!   "variables": ["big", "small"],
//!   "states": [
//!     {"action": null, "values": {"big": 0, "small": 0}},
//!     {"action": {"name": "FillBigJug", "location": "line 68, col 18 to line 69, col 34 of module DieHard"},
//!      "values": {"big": 5, "small": 0}}
//!   ],
//!   "ending": {"kind": "invariant", "name": "NotSolved"}
//! }
//! ```
//!
//! - `variables` lists the specification's variables. Every state assigns
//!   exactly these. Version 1 uses normalized value JSON with known stored
//!   representation gaps ([`crate::value_json`]); opt-in version 2 uses
//!   checked bounded stored representation ([`crate::value_repr`]).
//! - `states[k].action` labels the step into state `k`: `null` for the
//!   initial state, and optional elsewhere. `name` is the action's name as
//!   TLC prints it, without parameters; `location` is informative.
//! - `ending` says what the trace claims ([`Ending`]):
//!
//!   | `kind` | Fields | Claim |
//!   |---|---|---|
//!   | `invariant` | `name` | the invariant is false in the last state |
//!   | `property` | `name` | the action property is false on the last step (or, for a one-state trace, the implied initial predicate is false in it) |
//!   | `deadlock` | | the last state has no successor |
//!   | `behavior` | | nothing beyond being a behavior of the specification (implementation traces) |
//!   | `lasso` | `back_to` | the last state's successor is state `back_to` (0-based); a liveness counterexample |
//!   | `stuttering` | | the behavior stutters forever after the last state |
//!   | `assertion`, `error` | `ec` (`error`, optional) | expanding the last state fails |
//! - `length` (optional) is the claimed number of states; it must match.
//! - `properties` lists all temporal properties violated by a lasso or
//!   stuttering word. Legacy documents without names remain unsupported for
//!   liveness validation. An enclosing report's names must agree with these.
//!
//! `dive check --output json` embeds this object as `counterexample.trace`;
//! [`TraceDoc::from_json`] accepts the object itself, a document that
//! carries it as `counterexample`, or `dive check`'s report or its
//! `counterexample` object.
//!
//! # TLC's format
//!
//! TLC's `-dumpTrace json` document (`{"vars": [...], "counterexample":
//! {"state": [[1, {...}], ...], "action": [[[1, {...}], {"name": ...}, [2,
//! {...}]], ...]}}`) is read too. It carries no ending, so one must be
//! given ([`TraceDoc::with_ending`]), and its values are in TLC's lossy form,
//! which [`crate::tlc`] interprets against the model.

use serde_json::{Map, Value as J, json};

/// The format name in [`TraceDoc::to_json`].
pub const FORMAT: &str = "dive-trace";

/// The default legacy writer version. Readers accept versions 1 and 2;
/// representation-preserving version 2 is an explicit encoding option.
pub const VERSION: u64 = 1;

/// What a trace claims about its end.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ending {
    /// The named invariant is false in the last state.
    Invariant(String),
    /// The named action property is false on the last step; for a
    /// one-state trace, the named implied initial predicate is false in it.
    Property(String),
    /// The last state has no successor.
    Deadlock,
    /// No violation: the trace is a behavior of the specification.
    Behavior,
    /// A liveness counterexample that loops back to state `back_to`
    /// (0-based).
    Lasso {
        /// The state the last state's successor equals.
        back_to: usize,
    },
    /// A liveness counterexample that stutters after the last state.
    Stuttering,
    /// An assertion fails while the last state is expanded.
    Assertion,
    /// An evaluation error while the last state is expanded.
    Error {
        /// TLC's error class, if known (`EC` name).
        ec: Option<String>,
    },
}

impl Ending {
    /// The JSON form.
    #[must_use]
    pub fn to_json(&self) -> J {
        match self {
            Self::Invariant(n) => json!({"kind": "invariant", "name": n}),
            Self::Property(n) => json!({"kind": "property", "name": n}),
            Self::Deadlock => json!({"kind": "deadlock"}),
            Self::Behavior => json!({"kind": "behavior"}),
            Self::Lasso { back_to } => json!({"kind": "lasso", "back_to": back_to}),
            Self::Stuttering => json!({"kind": "stuttering"}),
            Self::Assertion => json!({"kind": "assertion"}),
            Self::Error { ec } => json!({"kind": "error", "ec": ec}),
        }
    }

    /// Reads the JSON form.
    ///
    /// # Errors
    /// For an unknown kind or a missing field.
    pub fn from_json(j: &J) -> Result<Self, String> {
        let kind = j
            .get("kind")
            .and_then(J::as_str)
            .ok_or("an ending needs a `kind`")?;
        let name = || {
            j.get("name")
                .and_then(J::as_str)
                .map(str::to_owned)
                .ok_or_else(|| format!("a `{kind}` ending needs a `name`"))
        };
        Ok(match kind {
            "invariant" => Self::Invariant(name()?),
            "property" => Self::Property(name()?),
            "deadlock" => Self::Deadlock,
            "behavior" => Self::Behavior,
            "lasso" => Self::Lasso {
                back_to: j
                    .get("back_to")
                    .and_then(J::as_u64)
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or("a `lasso` ending needs `back_to`")?,
            },
            "stuttering" => Self::Stuttering,
            "assertion" => Self::Assertion,
            "error" => Self::Error {
                ec: j.get("ec").and_then(J::as_str).map(str::to_owned),
            },
            other => return Err(format!("unknown ending kind `{other}`")),
        })
    }

    /// The ending of a TLC counterexample reported with error class `class`
    /// (TLC's `EC` name) and violated property `property`. A temporal
    /// property violation needs the trace's loop, which TLC reports
    /// separately: `back_to` is the state its lasso returns to (0-based), or
    /// `None` for stuttering.
    #[must_use]
    pub fn from_tlc(class: &str, property: Option<&str>, back_to: Option<usize>) -> Option<Self> {
        let named = || property.map(str::to_owned);
        Some(match class {
            "TLC_INVARIANT_VIOLATED_INITIAL" | "TLC_INVARIANT_VIOLATED_BEHAVIOR" => {
                Self::Invariant(named()?)
            }
            "TLC_PROPERTY_VIOLATED_INITIAL" | "TLC_ACTION_PROPERTY_VIOLATED_BEHAVIOR" => {
                Self::Property(named()?)
            }
            "TLC_DEADLOCK_REACHED" => Self::Deadlock,
            "TLC_TEMPORAL_PROPERTY_VIOLATED" => match back_to {
                Some(back_to) => Self::Lasso { back_to },
                None => Self::Stuttering,
            },
            "TLC_VALUE_ASSERT_FAILED" => Self::Assertion,
            other => Self::Error {
                ec: Some(other.to_owned()),
            },
        })
    }

    /// Parses the command-line form: `invariant:NAME`, `property:NAME`,
    /// `deadlock`, `behavior`, `lasso:K`, `stuttering`, `assertion`,
    /// `error[:EC]`.
    ///
    /// # Errors
    /// For anything else.
    pub fn parse(text: &str) -> Result<Self, String> {
        let (kind, arg) = match text.split_once(':') {
            Some((k, a)) => (k, Some(a)),
            None => (text, None),
        };
        let mut j = json!({ "kind": kind });
        if let Some(a) = arg {
            match kind {
                "lasso" => {
                    j["back_to"] = J::from(
                        a.parse::<u64>()
                            .map_err(|_| format!("`{a}` is not a state index"))?,
                    );
                }
                "error" => j["ec"] = J::from(a),
                _ => j["name"] = J::from(a),
            }
        }
        Self::from_json(&j)
    }
}

/// The label of a step.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Label {
    /// The action's name, without parameters.
    pub name: String,
    /// Its location, as given.
    pub location: Option<String>,
}

/// Trace-value transport selected explicitly by writers. Legacy output remains default.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TraceEncoding {
    /// Version 1 normalized JSON; known representation gaps remain.
    #[default]
    LegacyV1,
    /// Version 2 bounded checked stored-value transport.
    RepresentationV2,
}

/// Which format a document was read from.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Source {
    /// Legacy dive trace format version 1.
    Dive,
    /// Opt-in version 2: checked stored-value representation (ADR 0055).
    DiveRepresentation,
    /// TLC's `-dumpTrace json`: values are in TLC's lossy form.
    Tlc,
}

/// A counterexample as data, before its values are read against a model.
#[derive(Clone, Debug, PartialEq)]
pub struct TraceDoc {
    /// The variables, as listed by the document.
    pub variables: Vec<String>,
    /// Each state's values by variable name, in the document's JSON form.
    pub states: Vec<Map<String, J>>,
    /// The label of the step into each state (`labels[0]` is `None`).
    pub labels: Vec<Option<Label>>,
    /// What the trace claims, if the document says.
    pub ending: Option<Ending>,
    /// The claimed number of states, if the document says.
    pub length: Option<usize>,
    /// The format.
    pub source: Source,
    /// All temporal properties claimed to be false on this periodic word.
    /// Empty for other endings, or for a legacy unnamed liveness document.
    pub properties: Vec<String>,
}

impl TraceDoc {
    /// A behavior in dive's format, from states given as JSON objects
    /// (variable name to value, in dive's JSON form), with no labels. The
    /// ending is [`Ending::Behavior`]: use it to check an implementation's
    /// trace.
    ///
    /// # Errors
    /// If a state is not a JSON object.
    pub fn behavior(states: Vec<J>) -> Result<Self, String> {
        let mut out = Vec::with_capacity(states.len());
        for (k, s) in states.into_iter().enumerate() {
            match s {
                J::Object(m) => out.push(m),
                _ => return Err(format!("state {k} is not a JSON object")),
            }
        }
        let variables = out
            .first()
            .map(|m| m.keys().cloned().collect())
            .unwrap_or_default();
        let n = out.len();
        Ok(Self {
            variables,
            states: out,
            labels: vec![None; n],
            ending: Some(Ending::Behavior),
            length: None,
            source: Source::Dive,
            properties: Vec::new(),
        })
    }

    /// Checks opt-in version-2 word transport bounds and structural lengths.
    /// This never proves Init, Next, any property, or a completed search.
    /// Legacy version 1 retains its existing admission behavior.
    ///
    /// # Errors
    /// Stored transport byte/node/nesting/member or structural limits.
    pub fn admit_transport(&self) -> Result<(), String> {
        if self.source == Source::DiveRepresentation {
            crate::value_repr::word_admission(self)?;
        }
        Ok(())
    }

    /// This document with its ending replaced.
    #[must_use]
    pub fn with_ending(mut self, ending: Ending) -> Self {
        self.ending = Some(ending);
        self
    }

    /// Names every temporal property this periodic word claims to violate.
    /// Validation checks the names, their truth values and completeness.
    #[must_use]
    pub fn with_properties(mut self, properties: Vec<String>) -> Self {
        self.properties = properties;
        self
    }

    /// Reads a document in dive's format (or one that carries it as
    /// `counterexample`), or in TLC's `-dumpTrace json` format.
    ///
    /// # Errors
    /// A message naming what is malformed.
    pub fn from_json(j: &J) -> Result<Self, String> {
        if j.get("format").and_then(J::as_str) == Some(FORMAT) {
            return Self::from_dive(j);
        }
        // `dive check --output json`'s counterexample object carries the
        // document as `trace`; accept the object or the whole report.
        for t in [
            j.get("trace"),
            j.get("counterexample").and_then(|c| c.get("trace")),
        ]
        .into_iter()
        .flatten()
        {
            if t.get("format").and_then(J::as_str) == Some(FORMAT) {
                let mut doc = Self::from_dive(t)?;
                let outer = j.get("counterexample").unwrap_or(j);
                if let Some(names) = outer.get("properties") {
                    let names =
                        strings(Some(names)).ok_or("`properties` must be a list of names")?;
                    let outer_set: std::collections::BTreeSet<_> = names.iter().collect();
                    if outer_set.len() != names.len() {
                        return Err("enclosing report repeats a temporal property name".into());
                    }
                    let inner_set: std::collections::BTreeSet<_> = doc.properties.iter().collect();
                    if doc.properties.is_empty() {
                        doc.properties = names;
                    } else if inner_set != outer_set || inner_set.len() != doc.properties.len() {
                        return Err("enclosing report and trace disagree on `properties`".into());
                    }
                }
                return Ok(doc);
            }
        }
        if let Some(c) = j.get("counterexample") {
            if c.get("format").and_then(J::as_str) == Some(FORMAT) {
                return Self::from_dive(c);
            }
            if j.get("vars").is_some() {
                return Self::from_tlc(j);
            }
        }
        Err(format!(
            "not a trace: expected a `{FORMAT}` document or TLC's -dumpTrace json document"
        ))
    }

    /// Reads [`TraceDoc::from_json`] from text.
    ///
    /// # Errors
    /// For invalid JSON or a malformed document.
    pub fn parse(text: &str) -> Result<Self, String> {
        let j: J = serde_json::from_str(text).map_err(|e| format!("invalid JSON: {e}"))?;
        Self::from_json(&j)
    }

    fn from_dive(j: &J) -> Result<Self, String> {
        let version = j.get("version").and_then(J::as_u64);
        if !matches!(version, Some(1 | 2)) {
            return Err(format!(
                "unsupported {FORMAT} version {version:?}; this build reads versions 1 and 2"
            ));
        }
        if version == Some(2) {
            crate::value_repr::document_admission(j)?;
            let allowed = [
                "format",
                "version",
                "variables",
                "states",
                "ending",
                "length",
                "properties",
            ];
            if j.as_object()
                .is_none_or(|o| o.keys().any(|key| !allowed.contains(&key.as_str())))
            {
                return Err("stored trace has unknown document fields".into());
            }
        }
        let variables = strings(j.get("variables")).ok_or("`variables` must be a list of names")?;
        let states_j = j
            .get("states")
            .and_then(J::as_array)
            .ok_or("`states` must be a list")?;
        let mut states = Vec::with_capacity(states_j.len());
        let mut labels = Vec::with_capacity(states_j.len());
        for (k, s) in states_j.iter().enumerate() {
            let values = s
                .get("values")
                .and_then(J::as_object)
                .ok_or_else(|| format!("state {k} has no `values` object"))?;
            states.push(values.clone());
            labels.push(match s.get("action") {
                None | Some(J::Null) => None,
                Some(a) => Some(Label {
                    name: a
                        .get("name")
                        .and_then(J::as_str)
                        .ok_or_else(|| format!("state {k}: an action needs a `name`"))?
                        .to_owned(),
                    location: a.get("location").and_then(J::as_str).map(str::to_owned),
                }),
            });
        }
        let ending = match j.get("ending") {
            None | Some(J::Null) => None,
            Some(e) => Some(Ending::from_json(e)?),
        };
        let length = match j.get("length") {
            None | Some(J::Null) => None,
            Some(l) => Some(
                l.as_u64()
                    .and_then(|n| usize::try_from(n).ok())
                    .ok_or("`length` must be a count")?,
            ),
        };
        Ok(Self {
            variables,
            states,
            labels,
            ending,
            length,
            source: if version == Some(2) {
                Source::DiveRepresentation
            } else {
                Source::Dive
            },
            properties: match j.get("properties") {
                None => Vec::new(),
                Some(names) => {
                    strings(Some(names)).ok_or("`properties` must be a list of names")?
                }
            },
        })
    }

    fn from_tlc(j: &J) -> Result<Self, String> {
        let variables = strings(j.get("vars")).ok_or("`vars` must be a list of names")?;
        let cex = &j["counterexample"];
        let state_j = cex
            .get("state")
            .and_then(J::as_array)
            .ok_or("`counterexample.state` must be a list")?;
        let mut states = Vec::with_capacity(state_j.len());
        for (k, s) in state_j.iter().enumerate() {
            match s.as_array().map(Vec::as_slice) {
                Some([J::Number(n), J::Object(m)]) if n.as_u64() == Some(k as u64 + 1) => {
                    states.push(m.clone());
                }
                _ => {
                    return Err(format!(
                        "`counterexample.state[{k}]` is not [{}, {{...}}]",
                        k + 1
                    ));
                }
            }
        }
        let mut labels = vec![None; states.len()];
        let actions = cex
            .get("action")
            .and_then(J::as_array)
            .map(Vec::as_slice)
            .unwrap_or_default();
        let count = states.len();
        let state_at = |end: &J, at: usize| {
            matches!(end.as_array().map(Vec::as_slice),
                Some([J::Number(i), J::Object(m)])
                    if i.as_u64() == Some(at as u64 + 1) && states.get(at) == Some(m))
        };
        for (k, a) in actions.iter().enumerate() {
            let Some([from, label, to]) = a.as_array().map(Vec::as_slice) else {
                return Err(format!(
                    "`counterexample.action[{k}]` is not [from, label, to]"
                ));
            };
            if k + 1 == count {
                // A liveness counterexample's loop: from the last state back
                // to a listed one (itself, for stuttering). The ending
                // carries it; it must still connect listed states.
                let back = to
                    .get(0)
                    .and_then(J::as_u64)
                    .and_then(|i| usize::try_from(i).ok())
                    .filter(|&i| i >= 1 && i <= count);
                if !state_at(from, k) || !back.is_some_and(|i| state_at(to, i - 1)) {
                    return Err(format!(
                        "`counterexample.action[{k}]` is not a loop from the last state to a listed one"
                    ));
                }
                continue;
            }
            // TLC repeats the states around each step; they must be the
            // listed ones, or the document contradicts itself.
            for (end, at) in [(from, k), (to, k + 1)] {
                if !state_at(end, at) {
                    return Err(format!(
                        "`counterexample.action[{k}]` does not connect states {} and {}",
                        k + 1,
                        k + 2
                    ));
                }
            }
            labels[k + 1] = Some(Label {
                name: label
                    .get("name")
                    .and_then(J::as_str)
                    .ok_or_else(|| format!("`counterexample.action[{k}]` has no name"))?
                    .to_owned(),
                location: label.get("location").map(|l| match l {
                    J::Object(m) => format!(
                        "line {}, col {} to line {}, col {} of module {}",
                        m.get("beginLine").unwrap_or(&J::Null),
                        m.get("beginColumn").unwrap_or(&J::Null),
                        m.get("endLine").unwrap_or(&J::Null),
                        m.get("endColumn").unwrap_or(&J::Null),
                        m.get("module").and_then(J::as_str).unwrap_or("?"),
                    ),
                    other => other.to_string(),
                }),
            });
        }
        if !actions.is_empty() && actions.len() + 1 != count && actions.len() != count {
            return Err(format!(
                "{} states but {} steps",
                states.len(),
                actions.len()
            ));
        }
        Ok(Self {
            variables,
            states,
            labels,
            ending: None,
            length: None,
            source: Source::Tlc,
            properties: Vec::new(),
        })
    }

    /// Writes the document in dive's format. A document read from TLC's
    /// format keeps TLC's lossy values. This preserves the document JSON,
    /// without certifying concrete stored representation or semantic validity.
    /// [`crate::Trace::to_doc`] uses legacy normalized values;
    /// [`crate::Trace::to_doc_with_encoding`] can select bounded version 2.
    #[must_use]
    pub fn to_json(&self) -> J {
        let states: Vec<J> = self
            .states
            .iter()
            .zip(&self.labels)
            .map(|(s, l)| {
                json!({
                    "action": l.as_ref().map(|l| json!({"name": l.name, "location": l.location})),
                    "values": s,
                })
            })
            .collect();
        let mut j = json!({
            "format": FORMAT,
            "version": if self.source == Source::DiveRepresentation { 2 } else { VERSION },
            "variables": self.variables,
            "states": states,
            "ending": self.ending.as_ref().map(Ending::to_json),
        });
        if let Some(n) = self.length {
            j["length"] = J::from(n);
        }
        if !self.properties.is_empty() {
            j["properties"] = J::from(self.properties.clone());
        }
        j
    }
}

fn strings(j: Option<&J>) -> Option<Vec<String>> {
    j?.as_array()?
        .iter()
        .map(|v| v.as_str().map(str::to_owned))
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_tlc_dump_trace_json() -> Result<(), String> {
        let text = r#"{"counterexample":{"action":[[[1,{"x":0}],{"name":"Next","location":{"beginLine":9,"beginColumn":9,"endLine":9,"endColumn":18,"module":"M"}},[2,{"x":1}]]],"state":[[1,{"x":0}],[2,{"x":1}]]},"vars":["x"]}"#;
        let d = TraceDoc::parse(text)?;
        assert_eq!(d.source, Source::Tlc);
        assert_eq!(d.states.len(), 2);
        assert_eq!(d.labels[0], None);
        let l = d.labels[1].clone().ok_or("label")?;
        assert_eq!(l.name, "Next");
        assert_eq!(
            l.location.as_deref(),
            Some("line 9, col 9 to line 9, col 18 of module M")
        );
        Ok(())
    }

    #[test]
    fn rejects_a_tlc_document_whose_steps_do_not_connect() {
        let text = r#"{"counterexample":{"action":[[[1,{"x":0}],{"name":"Next"},[2,{"x":5}]]],"state":[[1,{"x":0}],[2,{"x":1}]]},"vars":["x"]}"#;
        assert!(TraceDoc::parse(text).is_err());
    }

    #[test]
    fn dive_format_round_trips() -> Result<(), String> {
        let text = r#"{"format":"dive-trace","version":1,"variables":["x"],
            "states":[{"action":null,"values":{"x":0}},{"action":{"name":"A","location":null},"values":{"x":1}}],
            "ending":{"kind":"invariant","name":"Inv"},"length":2}"#;
        let d = TraceDoc::parse(text)?;
        assert_eq!(d.ending, Some(Ending::Invariant("Inv".to_owned())));
        assert_eq!(TraceDoc::from_json(&d.to_json())?, d);
        // Embedded in a `dive check --output json` document.
        let wrapped = json!({"schema_version": 1, "counterexample": d.to_json()});
        assert_eq!(TraceDoc::from_json(&wrapped)?, d);
        Ok(())
    }

    #[test]
    fn parses_command_line_endings() -> Result<(), String> {
        assert_eq!(
            Ending::parse("invariant:Inv")?,
            Ending::Invariant("Inv".into())
        );
        assert_eq!(Ending::parse("deadlock")?, Ending::Deadlock);
        assert_eq!(Ending::parse("lasso:2")?, Ending::Lasso { back_to: 2 });
        assert!(Ending::parse("invariant").is_err());
        assert!(Ending::parse("nonsense").is_err());
        Ok(())
    }
}
