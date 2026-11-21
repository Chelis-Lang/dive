//! A trace read against a model: states as values, in the model's variable
//! order.

use std::sync::Arc;

use dive_model::Model;
use dive_value::Value;
use serde_json::{Map, Value as J};

use crate::format::{Ending, Label, Source, TraceDoc};
use crate::stream::StateSource;
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
    /// Recorded existential witnesses per state ([`TraceDoc::witnesses`]):
    /// empty, or one list per state.
    pub witnesses: Vec<Vec<Value>>,
    /// Recorded draws of random operators per state ([`TraceDoc::draws`]):
    /// empty, or one list per state.
    pub draws: Vec<Vec<Value>>,
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
        let lists = |lists: &[Vec<J>], what: &str| -> Result<Vec<Vec<Value>>, ShapeError> {
            let mut out = Vec::with_capacity(lists.len());
            for (k, values) in lists.iter().enumerate() {
                let values = values
                    .iter()
                    .map(|j| value_json::from_json(j, &model.names, ctx))
                    .collect::<Result<Vec<_>, _>>()
                    .map_err(|detail| ShapeError {
                        state: Some(k),
                        detail: format!("{what}: {detail}"),
                    })?;
                out.push(values);
            }
            Ok(out)
        };
        let witnesses = lists(&doc.witnesses, "a witness")?;
        let draws = lists(&doc.draws, "a draw")?;
        Ok(Self {
            states,
            labels: doc.labels.clone(),
            ending,
            length: doc.length,
            properties: doc.properties.clone(),
            concrete_closure: doc.concrete_closure,
            witnesses,
            draws,
        })
    }

    /// Reads the document made of `header` (whose own states are ignored)
    /// and the states of `source` against `model`, as [`Self::decode`] reads
    /// the whole document, decoding each state as it is produced.
    ///
    /// # Errors
    /// The first state that does not fit, or a missing ending.
    pub fn decode_source(
        model: &Model,
        header: &TraceDoc,
        source: &dyn StateSource,
    ) -> Result<Self, ShapeError> {
        if header.source == Source::Tlc {
            return Err(ShapeError {
                state: None,
                detail: "TLC's lossy JSON values must be read with dive_trace::tlc".to_owned(),
            });
        }
        let ending = header.ending.clone().ok_or_else(|| ShapeError {
            state: None,
            detail: "the trace does not say how it ends (no `ending`)".to_owned(),
        })?;
        let ctx = model.evaluator.options().ctx;
        let vars = variables(model);
        let n = source.len();
        let mut states = Vec::with_capacity(n);
        let mut labels = Vec::with_capacity(n);
        let mut witnesses: Vec<Vec<Value>> = Vec::new();
        let mut draws: Vec<Vec<Value>> = Vec::new();
        let mut failure = None;
        let produced = source.states(&mut |state| {
            let k = states.len();
            let read = |j: &J| value_json::from_json(j, &model.names, ctx);
            match decode_state(&vars, &state.values, |_, j| read(j)) {
                Ok(decoded) => states.push(decoded),
                Err(detail) => {
                    failure = Some(ShapeError {
                        state: Some(k),
                        detail: detail.clone(),
                    });
                    return Err(detail);
                }
            }
            labels.push(state.label);
            for (given, list, what) in [
                (&state.witnesses, &mut witnesses, "a witness"),
                (&state.draws, &mut draws, "a draw"),
            ] {
                if given.is_empty() {
                    continue;
                }
                match given.iter().map(read).collect::<Result<Vec<_>, _>>() {
                    Ok(values) => {
                        list.resize(n.max(k + 1), Vec::new());
                        list[k] = values;
                    }
                    Err(detail) => {
                        let detail = format!("{what}: {detail}");
                        failure = Some(ShapeError {
                            state: Some(k),
                            detail: detail.clone(),
                        });
                        return Err(detail);
                    }
                }
            }
            Ok(())
        });
        if let Some(failure) = failure {
            return Err(failure);
        }
        produced.map_err(|detail| ShapeError {
            state: Some(states.len()),
            detail,
        })?;
        if states.len() != n {
            return Err(ShapeError {
                state: None,
                detail: format!("the source claims {n} states but produced {}", states.len()),
            });
        }
        Ok(Self {
            states,
            labels,
            ending,
            length: header.length,
            properties: header.properties.clone(),
            concrete_closure: header.concrete_closure,
            witnesses,
            draws,
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
        let json = |lists: &[Vec<Value>]| {
            lists
                .iter()
                .map(|values| values.iter().map(|v| value_json::to_json(v, ctx)).collect())
                .collect::<Result<Vec<Vec<J>>, String>>()
        };
        let witnesses = json(&self.witnesses)?;
        let draws = json(&self.draws)?;
        Ok(TraceDoc {
            variables: vars,
            states,
            labels: self.labels.clone(),
            ending: Some(self.ending.clone()),
            length: self.length,
            source: Source::Dive,
            properties: self.properties.clone(),
            concrete_closure: self.concrete_closure,
            witnesses,
            draws,
            startup_draws: Vec::new(),
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
