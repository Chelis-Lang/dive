//! A trace read against a model: states as values, in the model's variable
//! order.

use std::sync::Arc;

use dive_model::Model;
use dive_value::Value;
use serde_json::{Map, Value as J};

use crate::format::{Ending, Label, Source, TraceDoc, TraceEncoding};
use crate::value_json;

/// A state: the value of each of the model's variables, by
/// [`dive_ir::VarId`]. Every entry is `Some`.
pub type State = Arc<[Option<Value>]>;

/// A counterexample (or behavior) read against a model.
#[derive(Clone, Debug)]
pub struct Trace {
    /// The states.
    pub states: Vec<State>,
    /// The label of the step into each state (`labels[0]` is `None`).
    pub labels: Vec<Option<Label>>,
    /// What the trace claims.
    pub ending: Ending,
    /// The claimed number of states, if given.
    pub length: Option<usize>,
    /// All temporal properties claimed false on a periodic word.
    pub properties: Vec<String>,
}

/// The model's variable names, by [`dive_ir::VarId`].
#[must_use]
pub fn variables(model: &Model) -> Vec<String> {
    model
        .ir
        .vars()
        .iter()
        .map(|&d| model.ir.decl(d).name.clone())
        .collect()
}

/// Why a document could not be read as a trace of the model: the state, and
/// what is wrong.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShapeError {
    /// The state (0-based), or `None` for the document as a whole.
    pub state: Option<usize>,
    /// What is wrong.
    pub detail: String,
}

impl Trace {
    /// Reads a [`Source::Dive`] document's values against `model`, checking
    /// that every state assigns exactly the model's variables. A
    /// [`Source::Tlc`] document is read by [`crate::tlc`] instead.
    ///
    /// # Errors
    /// The first state that does not fit, or a missing ending.
    pub fn decode(model: &Model, doc: &TraceDoc) -> Result<Self, ShapeError> {
        if doc.source == Source::Tlc {
            return Err(ShapeError {
                state: None,
                detail: "TLC's lossy JSON values must be read with dive_trace::tlc".to_owned(),
            });
        }
        if doc.source == Source::DiveRepresentation {
            doc.admit_transport().map_err(|detail| ShapeError {
                state: None,
                detail,
            })?;
        }
        let ending = doc.ending.clone().ok_or_else(|| ShapeError {
            state: None,
            detail: "the trace does not say how it ends (no `ending`)".to_owned(),
        })?;
        if doc.source == Source::DiveRepresentation {
            model.validate_context().map_err(|e| ShapeError {
                state: None,
                detail: e.to_string(),
            })?;
        }
        let ctx = model.evaluator.options().ctx;
        let vars = variables(model);
        if doc.source == Source::DiveRepresentation && doc.variables != vars {
            return Err(ShapeError {
                state: None,
                detail: "stored trace variable order/list differs from model".into(),
            });
        }
        let mut states = Vec::with_capacity(doc.states.len());
        for (k, s) in doc.states.iter().enumerate() {
            let state = decode_state(&vars, s, |_, j| {
                if doc.source == Source::DiveRepresentation {
                    crate::value_repr::from_json(j, &model.names, ctx)
                } else {
                    value_json::from_json(j, &model.names, ctx)
                }
            })
            .map_err(|detail| ShapeError {
                state: Some(k),
                detail,
            })?;
            states.push(state);
        }
        Ok(Self {
            states,
            labels: doc.labels.clone(),
            ending,
            length: doc.length,
            properties: doc.properties.clone(),
        })
    }

    /// The trace in the default legacy version-1 normalized JSON format.
    /// Stored distinctions are not all preserved; select
    /// [`Self::to_doc_with_encoding`] for bounded representation version 2.
    ///
    /// # Errors
    /// If a value has no JSON form (an infinite set).
    pub fn to_doc(&self, model: &Model) -> Result<TraceDoc, String> {
        self.to_doc_with_encoding(model, TraceEncoding::LegacyV1)
    }

    /// Writes a selected dive transport version. TLC lossy transport cannot be selected.
    ///
    /// # Errors
    /// Unsupported representation or transport, including version-2 bounded admission.
    pub fn to_doc_with_encoding(
        &self,
        model: &Model,
        encoding: TraceEncoding,
    ) -> Result<TraceDoc, String> {
        if encoding == TraceEncoding::RepresentationV2 {
            model.validate_context().map_err(|e| e.to_string())?;
            if self.states.len() > crate::value_repr::MAX_MEMBERS
                || self.labels.len() != self.states.len()
            {
                return Err("stored trace member or label-length limit".into());
            }
        }
        let ctx = model.evaluator.options().ctx;
        let vars = variables(model);
        let mut states = Vec::with_capacity(self.states.len());
        for s in &self.states {
            if encoding == TraceEncoding::RepresentationV2 && s.len() != vars.len() {
                return Err("stored state variable count disagrees with model".into());
            }
            let mut m = Map::new();
            for (name, v) in vars.iter().zip(s.iter()) {
                let v = v.as_ref().ok_or("a state has an unassigned variable")?;
                m.insert(
                    name.clone(),
                    if encoding == TraceEncoding::RepresentationV2 {
                        crate::value_repr::to_json(v, ctx)?
                    } else {
                        value_json::to_json(v, ctx)?
                    },
                );
            }
            states.push(m);
        }
        if encoding == TraceEncoding::RepresentationV2 {
            crate::value_repr::states_admission(&states)?;
        }
        let doc = TraceDoc {
            variables: vars,
            states,
            labels: self.labels.clone(),
            ending: Some(self.ending.clone()),
            length: self.length,
            source: if encoding == TraceEncoding::RepresentationV2 {
                Source::DiveRepresentation
            } else {
                Source::Dive
            },
            properties: self.properties.clone(),
        };
        doc.admit_transport()?;
        Ok(doc)
    }
}

/// Reads one state: exactly the variables `vars`, each value read by `read`.
pub(crate) fn decode_state(
    vars: &[String],
    s: &Map<String, J>,
    mut read: impl FnMut(&str, &J) -> Result<Value, String>,
) -> Result<State, String> {
    if let Some(extra) = s.keys().find(|k| !vars.contains(k)) {
        return Err(format!("`{extra}` is not a variable of the specification"));
    }
    let mut out = Vec::with_capacity(vars.len());
    for v in vars {
        let j = s
            .get(v)
            .ok_or_else(|| format!("variable `{v}` is not assigned"))?;
        out.push(Some(
            read(v, j).map_err(|e| format!("variable `{v}`: {e}"))?,
        ));
    }
    Ok(out.into())
}
