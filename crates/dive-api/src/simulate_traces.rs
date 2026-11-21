//! A sampled trace as the report and `--trace-dir` write it.
//!
//! Simulation already holds each trace's states as values, so the document
//! is never built as JSON: [`SimStates`] produces one state at a time for
//! writing ([`dive_trace::stream::write_document`]) and for independent
//! validation ([`crate::trace::SpecModel::validate_sample_source`]). Memory
//! for a trace is then its values, the validator's own decoded copy and the
//! written text, instead of several JSON trees of the whole trace.

use std::io::Write;

use dive_sim::Outcome;
use dive_trace::stream::{Measure, StateSource, Streamed, write_document};
use dive_trace::{Ending, Label, TraceDoc};
use serde_json::{Map, Value, json};

/// The states of a sampled trace, produced on demand in dive's JSON form.
pub(crate) struct SimStates<'a> {
    model: &'a dive_model::Model,
    trace: &'a dive_sim::Trace,
    /// Variable names in document order, with their index in a state.
    names: Vec<(String, usize)>,
    /// Returns true once the run's budget is exhausted.
    stop: &'a (dyn Fn() -> bool + Sync),
}

impl<'a> SimStates<'a> {
    pub(crate) fn new(
        model: &'a dive_model::Model,
        trace: &'a dive_sim::Trace,
        stop: &'a (dyn Fn() -> bool + Sync),
    ) -> Self {
        let mut names: Vec<(String, usize)> = model
            .ir
            .vars()
            .iter()
            .enumerate()
            .map(|(index, &var)| (model.ir.decl(var).name.clone(), index))
            .collect();
        names.sort();
        Self {
            model,
            trace,
            names,
            stop,
        }
    }

    /// The document around these states: its variables and `ending`, and
    /// for a lasso or stuttering word the properties false on it.
    pub(crate) fn header(&self, ending: Ending) -> TraceDoc {
        let mut header = TraceDoc::behavior(Vec::new())
            .unwrap_or_else(|_| unreachable!("an empty behavior is a document"))
            .with_ending(ending);
        header.variables = self.names.iter().map(|(name, _)| name.clone()).collect();
        // A lasso's loop closes on equal states, which the validator then
        // checks whatever VIEW or SYMMETRY is declared (D-037).
        if let Some((ending, properties)) = &self.trace.temporal
            && !matches!(ending, dive_sim::TemporalEnding::BadPrefix(_))
        {
            header = header
                .with_properties(properties.clone())
                .with_concrete_closure();
        }
        header
    }

    /// The whole document of `header` and these states, held in memory.
    /// Only a lasso or stuttering word is validated this way.
    fn document(&self, header: &TraceDoc) -> Result<TraceDoc, String> {
        let mut document = header.clone();
        self.states(&mut |state| {
            document.states.push(state.values);
            document.labels.push(state.label);
            document.witnesses.push(state.witnesses);
            Ok(())
        })?;
        if document.witnesses.iter().all(Vec::is_empty) {
            document.witnesses.clear();
        }
        Ok(document)
    }

    /// True for a lasso or stuttering word.
    fn periodic(&self) -> bool {
        matches!(
            &self.trace.temporal,
            Some((
                dive_sim::TemporalEnding::Lasso { .. } | dive_sim::TemporalEnding::Stuttering,
                _
            ))
        )
    }

    fn label(&self, state: usize) -> Option<Label> {
        let (name, location) = self.action(state);
        name.map(|name| Label { name, location })
    }

    /// The name, if it has one, and the source location of the action of
    /// the step into state `state`.
    pub(crate) fn action(&self, state: usize) -> (Option<String>, Option<String>) {
        let action = state
            .checked_sub(1)
            .and_then(|step| self.trace.actions.get(step))
            .and_then(|&index| self.model.actions.get(index));
        action.map_or((None, None), |action| {
            (
                crate::core_engine::action_name(self.model, action),
                Some(action.label().to_owned()),
            )
        })
    }

    /// Returns true once the run's budget is exhausted.
    pub(crate) fn stop(&self) -> &(dyn Fn() -> bool + Sync) {
        self.stop
    }

    /// State `index`'s values in dive's JSON form.
    pub(crate) fn values(&self, index: usize) -> Result<Map<String, Value>, String> {
        let ctx = self.model.evaluator.options().ctx;
        let state = self
            .trace
            .states
            .get(index)
            .ok_or_else(|| "a state past the trace".to_owned())?;
        let mut values = Map::new();
        for (name, at) in &self.names {
            let value = state
                .get(*at)
                .and_then(Option::as_ref)
                .ok_or_else(|| "incomplete sampled trace".to_owned())?;
            values.insert(name.clone(), dive_trace::value_json::to_json(value, ctx)?);
        }
        Ok(values)
    }
}

impl StateSource for SimStates<'_> {
    fn len(&self) -> usize {
        self.trace.states.len()
    }

    fn states(&self, each: &mut dyn FnMut(Streamed) -> Result<(), String>) -> Result<(), String> {
        let ctx = self.model.evaluator.options().ctx;
        let mut drawn = self.trace.witnesses.iter().peekable();
        for index in 0..self.trace.states.len() {
            if (self.stop)() {
                return Err("run budget exhausted during trace serialization".to_owned());
            }
            let mut witnesses = Vec::new();
            while let Some((state, members)) = drawn.next_if(|(state, _)| *state <= index) {
                if *state == index {
                    for member in members {
                        witnesses.push(dive_trace::value_json::to_json(member, ctx)?);
                    }
                }
            }
            each(Streamed {
                values: self.values(index)?,
                label: self.label(index),
                witnesses,
            })?;
        }
        Ok(())
    }
}

/// How `trace`, number `number` of the report, ends under the run's
/// `outcome`.
pub(crate) fn ending(outcome: &Outcome, number: usize, trace: &dive_sim::Trace) -> Ending {
    match outcome {
        _ if trace.temporal.is_some() => deciding_ending(outcome, trace),
        Outcome::Violation {
            trace: deciding, ..
        } if *deciding == number => deciding_ending(outcome, trace),
        _ => Ending::Behavior,
    }
}

/// How `trace`, which decided `outcome`, ends: its temporal word, its
/// violation, or a behavior.
pub(crate) fn deciding_ending(outcome: &Outcome, trace: &dive_sim::Trace) -> Ending {
    if let Some((ending, _)) = &trace.temporal {
        return match ending {
            dive_sim::TemporalEnding::Lasso { back_to } => Ending::Lasso { back_to: *back_to },
            dive_sim::TemporalEnding::Stuttering => Ending::Stuttering,
            dive_sim::TemporalEnding::BadPrefix(name) => Ending::BadPrefix(name.clone()),
        };
    }
    match outcome {
        Outcome::Violation { kind, name, .. } => match *kind {
            "invariant" => Ending::Invariant(name.clone().unwrap_or_default()),
            "property" => Ending::Property(name.clone().unwrap_or_default()),
            _ => Ending::Deadlock,
        },
        _ => Ending::Behavior,
    }
}

/// Independently validates `source`'s trace, whose document is `header`:
/// a sample or a finite violation (`sample`) as simulation found it, or a
/// diagnostic error prefix as an ordinary finite word. A lasso or stuttering
/// word is checked whole on a fresh session: every step including the
/// closing one, fairness, and the complete false-property set.
///
/// # Errors
/// An interrupted validation; the inner error is why the word could not be
/// built.
pub(crate) fn validate(
    model: &crate::trace::SpecModel,
    header: &TraceDoc,
    source: &SimStates<'_>,
    sample: bool,
    control: std::sync::Arc<dyn dive_eval::HostControl>,
) -> Result<Result<dive_trace::Report, String>, dive_trace::ValidationRunError> {
    if source.periodic() {
        return Ok(source
            .document(header)
            .map(|document| model.validate(&document)));
    }
    if sample {
        model
            .validate_sample_source(header, source, control)
            .map(Ok)
    } else {
        // Diagnostic error prefixes can include the actual failing row.
        // They carry no claim that every safety obligation passed there.
        model.validate_source(header, source, control).map(Ok)
    }
}

/// The indices of the states that differ from the state before them (the
/// first always), as TLC prints a trace.
pub(crate) fn distinct_states(
    model: &dive_model::Model,
    trace: &dive_sim::Trace,
    stop: &dyn Fn() -> bool,
) -> Result<Vec<usize>, String> {
    let mut kept = Vec::<usize>::new();
    for (index, state) in trace.states.iter().enumerate() {
        if stop() {
            return Err("run budget exhausted during trace compression".to_owned());
        }
        let same = if let Some(&previous) = kept.last() {
            let mut equal = true;
            for (left, right) in trace.states[previous].iter().zip(state.iter()) {
                if let (Some(left), Some(right)) = (left, right) {
                    equal &= model
                        .evaluator
                        .equal_values(left, right)
                        .map_err(|error| format!("simulation trace compression: {error}"))?;
                }
            }
            equal
        } else {
            false
        };
        if !same {
            kept.push(index);
        }
    }
    Ok(kept)
}

/// A report entry being written to `W`: its text up to the document, then
/// the document, then the rest once the trace is validated.
pub(crate) struct Entry<W: Write> {
    out: W,
    id: u64,
    length: usize,
    ending: Value,
    document_sha256: String,
}

/// One kept trace's report entry and the summary that replaces it when the
/// report byte bound leaves no room for it.
#[derive(Debug)]
pub(crate) struct Written {
    pub(crate) text: String,
    pub(crate) summary: String,
}

/// Writes both to `out` and to a [`Measure`].
struct Tee<'a, W: Write> {
    out: &'a mut W,
    measure: &'a mut Measure,
}

impl<W: Write> Write for Tee<'_, W> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.out.write_all(bytes)?;
        self.measure.write_all(bytes)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        self.out.flush()
    }
}

impl<W: Write> Entry<W> {
    /// Starts the entry of `source`'s trace on `out`: the `ALIAS`
    /// rendering's JSON text, if the model has one (`null` when it could not
    /// be rendered), then the document, written state by state.
    ///
    /// # Errors
    /// Why a state cannot be written (a value with no JSON form, the
    /// budget, or the writer).
    pub(crate) fn begin(
        mut out: W,
        header: &TraceDoc,
        source: &SimStates<'_>,
        alias: Option<&str>,
    ) -> Result<Self, String> {
        let io = |error: std::io::Error| error.to_string();
        out.write_all(b"{").map_err(io)?;
        if let Some(alias) = alias {
            out.write_all(b"\"alias\":").map_err(io)?;
            out.write_all(alias.as_bytes()).map_err(io)?;
            out.write_all(b",").map_err(io)?;
        }
        out.write_all(b"\"document\":").map_err(io)?;
        let mut measure = Measure::default();
        write_document(
            header,
            source,
            &mut Tee {
                out: &mut out,
                measure: &mut measure,
            },
        )
        .map_err(io)?;
        Ok(Self {
            out,
            id: source.trace.id,
            length: source.len(),
            ending: header.ending.as_ref().map_or(Value::Null, Ending::to_json),
            document_sha256: measure.sha256_hex(),
        })
    }

    /// Completes the entry with the states TLC would print (`tlc`) and the
    /// trace's validation, and returns the writer and the entry's summary.
    ///
    /// # Errors
    /// Why a state cannot be written.
    pub(crate) fn finish(
        mut self,
        source: &SimStates<'_>,
        tlc: &[usize],
        validation: &Value,
    ) -> Result<(W, String), String> {
        let io = |error: std::io::Error| error.to_string();
        let id = self.id;
        write!(self.out, ",\"id\":{id},\"tlc_states\":[").map_err(io)?;
        for (n, &index) in tlc.iter().enumerate() {
            if n > 0 {
                self.out.write_all(b",").map_err(io)?;
            }
            let values = Value::Object(source.values(index)?);
            serde_json::to_writer(&mut self.out, &values).map_err(|error| error.to_string())?;
        }
        self.out.write_all(b"],\"validation\":").map_err(io)?;
        serde_json::to_writer(&mut self.out, validation).map_err(|error| error.to_string())?;
        self.out.write_all(b"}").map_err(io)?;
        let summary = json!({
            "id": id,
            "summary": {
                "reason": "report byte bound",
                "length": self.length,
                "ending": self.ending,
                "document_sha256": self.document_sha256,
            },
            "validation": validation,
        })
        .to_string();
        Ok((self.out, summary))
    }
}

impl Entry<Vec<u8>> {
    /// [`Self::finish`] of an entry written to memory, as the report keeps it.
    pub(crate) fn finish_text(
        self,
        source: &SimStates<'_>,
        tlc: &[usize],
        validation: &Value,
    ) -> Result<Written, String> {
        let (text, summary) = self.finish(source, tlc, validation)?;
        let text = String::from_utf8(text).map_err(|error| error.to_string())?;
        Ok(Written { text, summary })
    }
}
