//! `--trace-dir`: every sampled trace written to its own file as it
//! commits, in id order, after independent validation.
//!
//! Workers validate and write each finished trace to a temporary file; the
//! commit renames it in id order, so the directory only ever shows a prefix
//! of the run's traces, and traces after the deciding one are removed. A
//! worker holds one trace at a time, so memory does not grow with the number
//! of traces.
//!
//! Formats:
//! - [`TraceFormat::Dive`]: the trace's report entry (`alias`, `document`,
//!   `id`, `tlc_states`, `validation`), which `dive validate-trace` reads.
//! - [`TraceFormat::TlcJson`] and [`TraceFormat::TlcTla`]: the layouts of
//!   TLC's `-dumpTrace json` and `-dumpTrace tla` (captured from the pinned
//!   oracle), with the `ALIAS` record in place of each state when the model
//!   has one, as TLC writes its violating trace. A state record lists the
//!   variables in declaration order; TLC's order follows its internal name
//!   table, which no reader depends on.

use std::collections::BTreeSet;
use std::io::{BufWriter, Write};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, PoisonError};

use dive_sim::Outcome;
use dive_value::{Ctx, Value};

use crate::simulate_traces::{Entry, SimStates};
use crate::trace::SpecModel;

/// The file format of `--trace-dir` traces.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum TraceFormat {
    /// dive's format: each file is the trace's report entry, whose
    /// `document` `dive validate-trace` reads.
    #[default]
    Dive,
    /// TLC's `-dumpTrace json` layout.
    TlcJson,
    /// TLC's `-dumpTrace tla` layout: a module defining `CounterExample`.
    TlcTla,
}

impl TraceFormat {
    /// The name `--trace-format` takes.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Dive => "dive",
            Self::TlcJson => "json",
            Self::TlcTla => "tla",
        }
    }

    const fn extension(self) -> &'static str {
        match self {
            Self::Dive | Self::TlcJson => "json",
            Self::TlcTla => "tla",
        }
    }
}

/// The file name of trace `id`: `trace_<id>.json` or `trace_<id>.tla`, so a
/// TLA+ file's module name, `trace_<id>`, is its file name.
#[must_use]
pub fn trace_file_name(id: u64, format: TraceFormat) -> String {
    format!("trace_{id}.{}", format.extension())
}

/// Writes validated traces into a directory.
pub(crate) struct TraceDir<'a> {
    dir: PathBuf,
    format: TraceFormat,
    spec: &'a SpecModel,
    control: Arc<dyn dive_eval::HostControl>,
    stop: &'a (dyn Fn() -> bool + Sync),
    /// Written to a temporary file, not yet committed.
    pending: Mutex<BTreeSet<u64>>,
    /// Committed.
    written: Mutex<BTreeSet<u64>>,
    /// Why a trace's `ALIAS` could not be rendered.
    alias_failures: Mutex<Vec<String>>,
    /// Rendering installs its own host on the model's evaluator, so workers
    /// render one at a time. Sampling itself never uses that evaluator.
    alias_lock: Mutex<()>,
}

impl<'a> TraceDir<'a> {
    /// Prepares `dir`, which must be absent or empty, so that every file in
    /// it after the run belongs to the run.
    pub(crate) fn create(
        dir: &Path,
        format: TraceFormat,
        spec: &'a SpecModel,
        control: Arc<dyn dive_eval::HostControl>,
        stop: &'a (dyn Fn() -> bool + Sync),
    ) -> Result<Self, String> {
        match std::fs::read_dir(dir) {
            Ok(mut entries) => {
                if entries.next().is_some() {
                    return Err(format!(
                        "--trace-dir {} is not empty; give an absent or empty directory",
                        dir.display()
                    ));
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
                std::fs::create_dir_all(dir).map_err(|error| {
                    format!("cannot create --trace-dir {}: {error}", dir.display())
                })?;
            }
            Err(error) => {
                return Err(format!(
                    "cannot read --trace-dir {}: {error}",
                    dir.display()
                ));
            }
        }
        Ok(Self {
            dir: dir.to_owned(),
            format,
            spec,
            control,
            stop,
            pending: Mutex::new(BTreeSet::new()),
            written: Mutex::new(BTreeSet::new()),
            alias_failures: Mutex::new(Vec::new()),
            alias_lock: Mutex::new(()),
        })
    }

    fn path(&self, id: u64, temporary: bool) -> PathBuf {
        let name = trace_file_name(id, self.format);
        self.dir.join(if temporary {
            format!(".{name}.partial")
        } else {
            name
        })
    }

    /// The number of traces written.
    pub(crate) fn count(&self) -> u64 {
        self.written
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .len() as u64
    }

    /// Whether trace `id` was written.
    pub(crate) fn has(&self, id: u64) -> bool {
        self.written
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .contains(&id)
    }

    /// Why some traces' `ALIAS` could not be rendered.
    pub(crate) fn alias_failures(&self) -> Vec<String> {
        std::mem::take(
            &mut self
                .alias_failures
                .lock()
                .unwrap_or_else(PoisonError::into_inner),
        )
    }

    /// Removes the temporary files of traces that were never committed.
    pub(crate) fn finish(&self) {
        let pending =
            std::mem::take(&mut *self.pending.lock().unwrap_or_else(PoisonError::into_inner));
        for id in pending {
            let _ = std::fs::remove_file(self.path(id, true));
        }
    }
}

impl TraceDir<'_> {
    /// Writes a validated trace to its temporary file.
    fn write_file(
        &self,
        trace: &dive_sim::Trace,
        source: &SimStates<'_>,
        header: &dive_trace::TraceDoc,
        validation: &serde_json::Value,
    ) -> Result<(), String> {
        let model = self.spec.model();
        let alias = model.alias.as_ref().and_then(|alias| alias.as_ref().ok());
        let alias_failed = |reason: String| {
            self.alias_failures
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(format!("trace {}: {reason}", trace.id));
        };
        let file = std::fs::File::create(self.path(trace.id, true))
            .map_err(|error| format!("cannot create the file: {error}"))?;
        self.pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(trace.id);
        let mut out = BufWriter::new(file);
        match self.format {
            TraceFormat::Dive => {
                let text = alias.map(|alias| {
                    let _rendering = self
                        .alias_lock
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    crate::simulate_alias::render(model, alias, &trace.states, self.stop)
                        .map_err(alias_failed)
                        .ok()
                });
                let tlc = crate::simulate_traces::distinct_states(model, trace, self.stop)?;
                Entry::begin(
                    &mut out,
                    header,
                    source,
                    text.as_ref().map(|text| text.as_deref().unwrap_or("null")),
                )
                .and_then(|entry| entry.finish(source, &tlc, validation))
                .map(|_| ())
            }
            TraceFormat::TlcJson | TraceFormat::TlcTla => {
                let records = alias.and_then(|alias| {
                    let _rendering = self
                        .alias_lock
                        .lock()
                        .unwrap_or_else(PoisonError::into_inner);
                    crate::simulate_alias::render_values(model, alias, &trace.states, self.stop)
                        .map_err(alias_failed)
                        .ok()
                });
                let module = format!("trace_{}", trace.id);
                write_tlc(
                    &mut out,
                    self.format,
                    &module,
                    source,
                    model,
                    trace,
                    records.as_deref(),
                )
            }
        }?;
        out.flush().map_err(|error| error.to_string())
    }
}

impl crate::core_modes::sample::TraceExport for TraceDir<'_> {
    fn write(&self, trace: &dive_sim::Trace, deciding: Option<&Outcome>) -> Result<(), Outcome> {
        let model = self.spec.model();
        let source = SimStates::new(model, trace, self.stop);
        let header = source.header(deciding.map_or(dive_trace::Ending::Behavior, |outcome| {
            crate::simulate_traces::deciding_ending(outcome, trace)
        }));
        let refused = |what: &str, why: String| {
            Outcome::Unsupported(format!("--trace-dir trace {}: {what}: {why}", trace.id))
        };
        // Every state must have a JSON form before validation reads it.
        dive_trace::stream::write_document(
            &header,
            &source,
            &mut dive_trace::stream::Measure::default(),
        )
        .map_err(|error| refused("serialization", error.to_string()))?;
        // A sample or a violation must validate, as in the report; the
        // prefix of a run-deciding error is written with its verdict, which
        // the report judges.
        let sample = matches!(deciding, None | Some(Outcome::Violation { .. }));
        let validated = crate::simulate_traces::validate(
            self.spec,
            &header,
            &source,
            sample,
            self.control.clone(),
        )
        .map_err(|error| refused("validation interrupted", error.to_string()))?
        .map_err(|error| refused("serialization", error))?;
        let validation = crate::trace::report_json(&validated);
        match validation["result"].as_str() {
            _ if !sample => {}
            Some("valid") => {}
            Some("invalid") => {
                return Err(Outcome::Internal(format!(
                    "independent simulation trace validation rejected trace {}",
                    trace.id
                )));
            }
            _ => {
                return Err(Outcome::Unsupported(format!(
                    "independent simulation trace validation: trace {}: {}",
                    trace.id,
                    validation["unsupported"].as_str().unwrap_or("unsupported")
                )));
            }
        }
        self.write_file(trace, &source, &header, &validation)
            .map_err(|why| refused("write", why))
    }

    fn commit(&self, id: u64) -> Result<(), String> {
        let was_pending = self
            .pending
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&id);
        if !was_pending {
            return Ok(());
        }
        std::fs::rename(self.path(id, true), self.path(id, false))
            .map_err(|error| format!("--trace-dir trace {id}: {error}"))?;
        self.written
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(id);
        Ok(())
    }
}

/// An action's name and location as TLC dumps them.
fn tlc_action(name: Option<&str>, location: Option<&str>) -> (String, Option<[String; 5]>) {
    let name = name.unwrap_or_default().to_owned();
    // dive's labels read `<Name line L, col C to line L2, col C2 of module M>`.
    let location = location.and_then(|text| {
        let text = text.trim_start_matches('<').trim_end_matches('>');
        let rest = &text[text.find(" line ")? + " line ".len()..];
        let (begin_line, rest) = rest.split_once(", col ")?;
        let (begin_column, rest) = rest.split_once(" to line ")?;
        let (end_line, rest) = rest.split_once(", col ")?;
        let (end_column, module) = rest.split_once(" of module ")?;
        let numbers = [begin_line, begin_column, end_line, end_column];
        numbers
            .iter()
            .all(|n| !n.is_empty() && n.bytes().all(|b| b.is_ascii_digit()))
            .then(|| {
                [
                    begin_line.to_owned(),
                    begin_column.to_owned(),
                    end_line.to_owned(),
                    end_column.to_owned(),
                    module.to_owned(),
                ]
            })
    });
    (name, location)
}

/// TLC's string syntax for a string.
fn tla_string(text: &str) -> String {
    let mut out = String::with_capacity(text.len() + 2);
    out.push('"');
    for c in text.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '\r' => out.push_str("\\r"),
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

/// One state as TLC dumps it: the `ALIAS` record when there is one, else
/// the variables as a record.
fn tlc_state(
    format: TraceFormat,
    model: &dive_model::Model,
    trace: &dive_sim::Trace,
    records: Option<&[Value]>,
    index: usize,
    ctx: Ctx,
) -> Result<String, String> {
    let fields: Vec<(String, &Value)> = if let Some(fields) = records
        .and_then(|records| records.get(index))
        .and_then(Value::as_fcn)
        .and_then(|record| record.as_record())
    {
        fields
            .iter()
            .map(|(atom, value)| (atom.as_str().to_owned(), value))
            .collect()
    } else {
        let state = trace.states.get(index).ok_or("a state past the trace")?;
        model
            .ir
            .vars()
            .iter()
            .zip(state.iter())
            .map(|(&var, value)| {
                value
                    .as_ref()
                    .map(|value| (model.ir.decl(var).name.clone(), value))
                    .ok_or_else(|| "incomplete sampled trace".to_owned())
            })
            .collect::<Result<_, _>>()?
    };
    let mut out = String::new();
    if format == TraceFormat::TlcJson {
        out.push('{');
        for (n, (name, value)) in fields.iter().enumerate() {
            if n > 0 {
                out.push(',');
            }
            out.push_str(&dive_value::json::json_string(name));
            out.push(':');
            out.push_str(&dive_value::json::to_tlc_json(value, ctx).map_err(|e| e.to_string())?);
        }
        out.push('}');
    } else {
        out.push('[');
        for (n, (name, value)) in fields.iter().enumerate() {
            if n > 0 {
                out.push_str(", ");
            }
            out.push_str(name);
            out.push_str(" |-> ");
            out.push_str(&dive_value::print::to_tlc_string(value, ctx).map_err(|e| e.to_string())?);
        }
        out.push(']');
    }
    Ok(out)
}

/// The variable names TLC lists: the `ALIAS` record's fields, or the
/// variables in declaration order.
fn tlc_vars(model: &dive_model::Model, records: Option<&[Value]>) -> Vec<String> {
    records
        .and_then(|records| records.first())
        .and_then(Value::as_fcn)
        .and_then(|record| record.as_record())
        .map_or_else(
            || {
                model
                    .ir
                    .vars()
                    .iter()
                    .map(|&var| model.ir.decl(var).name.clone())
                    .collect()
            },
            |fields| {
                fields
                    .iter()
                    .map(|(atom, _)| atom.as_str().to_owned())
                    .collect()
            },
        )
}

/// Writes a trace in TLC's `-dumpTrace json` or `tla` layout.
fn write_tlc(
    out: &mut dyn Write,
    format: TraceFormat,
    module: &str,
    source: &SimStates<'_>,
    model: &dive_model::Model,
    trace: &dive_sim::Trace,
    records: Option<&[Value]>,
) -> Result<(), String> {
    let ctx = model.evaluator.options().ctx;
    let json = format == TraceFormat::TlcJson;
    let io = |error: std::io::Error| error.to_string();
    let n = trace.states.len();
    if json {
        out.write_all(b"{\"counterexample\":{\"action\":[")
            .map_err(io)?;
    } else {
        write!(
            out,
            "---- MODULE {module} ----\nLOCAL INSTANCE TLC\n\nCounterExample == \n\t[action |-> {{"
        )
        .map_err(io)?;
    }
    let mut previous = if n > 0 {
        Some(tlc_state(format, model, trace, records, 0, ctx)?)
    } else {
        None
    };
    for index in 1..n {
        if (source.stop())() {
            return Err("run budget exhausted during trace serialization".to_owned());
        }
        let state = tlc_state(format, model, trace, records, index, ctx)?;
        let before = previous.as_deref().unwrap_or_default();
        let (name, location) = source.action(index);
        let (name, location) = tlc_action(name.as_deref(), location.as_deref());
        if json {
            let location = location.map_or_else(String::new, |[bl, bc, el, ec, m]| {
                format!(
                    ",\"location\":{{\"beginLine\":{bl},\"beginColumn\":{bc},\"endLine\":{el},\"endColumn\":{ec},\"module\":{}}}",
                    dive_value::json::json_string(&m)
                )
            });
            write!(
                out,
                "{}[[{index},{before}],{{\"name\":{}{location}}},[{},{state}]]",
                if index > 1 { "," } else { "" },
                dive_value::json::json_string(&name),
                index + 1
            )
            .map_err(io)?;
        } else {
            let location = location.map_or_else(String::new, |[bl, bc, el, ec, m]| {
                format!(
                    ", location |-> [beginLine |-> {bl}, beginColumn |-> {bc}, endLine |-> {el}, endColumn |-> {ec}, module |-> {}]",
                    tla_string(&m)
                )
            });
            write!(
                out,
                "{}<<<<{index}, {before}>>, [name |-> {}{location}], <<{}, {state}>>>>",
                if index > 1 { ", " } else { "" },
                tla_string(&name),
                index + 1
            )
            .map_err(io)?;
        }
        previous = Some(state);
    }
    if json {
        out.write_all(b"],\"state\":[").map_err(io)?;
    } else {
        out.write_all(b"}, state |-> {").map_err(io)?;
    }
    for index in 0..n {
        let state = tlc_state(format, model, trace, records, index, ctx)?;
        let separator = match (index, json) {
            (0, _) => "",
            (_, true) => ",",
            (_, false) => ", ",
        };
        if json {
            write!(out, "{separator}[{},{state}]", index + 1).map_err(io)?;
        } else {
            write!(out, "{separator}<<{}, {state}>>", index + 1).map_err(io)?;
        }
    }
    if json {
        let vars: Vec<String> = tlc_vars(model, records)
            .iter()
            .map(|name| dive_value::json::json_string(name))
            .collect();
        write!(out, "]}},\"vars\":[{}]}}", vars.join(",")).map_err(io)?;
    } else {
        out.write_all(b"}]\n\n====").map_err(io)?;
    }
    Ok(())
}
