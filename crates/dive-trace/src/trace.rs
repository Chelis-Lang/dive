//! A trace read against a model: states as values, in the model's variable
//! order.

use std::sync::Arc;

use dive_model::Model;
use dive_value::Value;
use serde_json::{Map, Value as J};

use crate::format::{Ending, Label, Source, TraceDoc};
use crate::value_json;

/// A state: the value of each of the model's variables, by
/// [`dive_ir::VarId`]. Every entry is `Some`. Names must have the text and token
/// of the model's interner; public typed inputs are checked before evaluation.
pub type State = Arc<[Option<Value>]>;

/// Checks public typed state values before any predicate can compare names.
pub(crate) fn validate_state_names(model: &Model, state: &State) -> Result<(), String> {
    for value in state.iter().flatten() {
        value
            .validate_names_in(&model.names, model.evaluator.options().ctx.order)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}

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
    /// The loop closes on concrete state equality ([`TraceDoc::concrete_closure`]).
    pub concrete_closure: bool,
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
    /// Logical length, counting a separate partial terminal candidate.
    #[must_use]
    pub fn logical_length(&self) -> usize {
        self.states.len().saturating_add(usize::from(matches!(
            self.ending,
            Ending::PartialNext { .. } | Ending::PartialInit { .. }
        )))
    }
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
        let ending = doc.ending.clone().ok_or_else(|| ShapeError {
            state: None,
            detail: "the trace does not say how it ends (no `ending`)".to_owned(),
        })?;
        let ctx = model.evaluator.options().ctx;
        let vars = variables(model);
        let mut states = Vec::with_capacity(doc.states.len());
        for (k, s) in doc.states.iter().enumerate() {
            let state = decode_state(&vars, s, |_, j| value_json::from_json(j, &model.names, ctx))
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
            concrete_closure: doc.concrete_closure,
        })
    }

    /// The trace in dive's format.
    ///
    /// # Errors
    /// If a value has no JSON form (an infinite set).
    pub fn to_doc(&self, model: &Model) -> Result<TraceDoc, String> {
        let ctx = model.evaluator.options().ctx;
        let vars = variables(model);
        let mut states = Vec::with_capacity(self.states.len());
        for s in &self.states {
            let mut m = Map::new();
            for (name, v) in vars.iter().zip(s.iter()) {
                let v = v.as_ref().ok_or("a state has an unassigned variable")?;
                m.insert(name.clone(), value_json::to_json(v, ctx)?);
            }
            states.push(m);
        }
        Ok(TraceDoc {
            variables: vars,
            states,
            labels: self.labels.clone(),
            ending: Some(self.ending.clone()),
            length: self.length,
            source: Source::Dive,
            properties: self.properties.clone(),
            concrete_closure: self.concrete_closure,
        })
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
