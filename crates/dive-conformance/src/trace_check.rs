//! Independent validation of counterexamples (ADR 0016 §3).
//!
//! The harness never trusts an implementation's own claim that its
//! counterexample is valid. For an implementation that is not an oracle, it
//! reads the counterexample document from the result, binds the model from
//! the same specification, configuration and profile, and runs `dive-trace`'s
//! validator in-process ([`validate_counterexample`]). The result is what
//! [`crate::implementation::Implementation::trace_validated`] reports.
//!
//! The same machinery runs the validator's controls over the goldens
//! ([`golden_controls`], `dive-conformance trace-controls`): every TLC
//! counterexample must validate (positive controls), and every
//! by-construction mutant of it must be rejected (negative controls).
//!
//! **Deadlock.** The validator cannot check that a state has no successor
//! without the successor enumerator, and refuses by name (ADR 0016,
//! amendment 1). The harness then accepts a deadlock counterexample only if
//! every other check passed *and* its last state equals the last state of
//! the golden's own counterexample, which the oracle reported as
//! deadlocked: the oracle, not the implementation, vouches for the claim.

use std::path::{Path, PathBuf};

use dive_api::trace::{Controls, Ending, SpecModel, TraceDoc, TraceOptions, Validation};

use crate::corpus::ModelEntry;
use crate::tlc::result::{OracleResult, TraceEnding};

/// The validator's answer for one counterexample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceCheck {
    /// Whether the counterexample validated.
    pub validated: bool,
    /// What the validator said.
    pub detail: String,
}

impl TraceCheck {
    fn no(detail: impl Into<String>) -> Self {
        Self {
            validated: false,
            detail: detail.into(),
        }
    }
}

/// The ending a result reports, in the validator's terms.
#[must_use]
pub fn ending_of(result: &OracleResult) -> Option<Ending> {
    let trace = result.trace.as_ref()?;
    let back_to = match trace.ending {
        TraceEnding::Lasso { back_to } => back_to
            .and_then(|b| usize::try_from(b).ok())
            .map(|b| b.saturating_sub(1)),
        TraceEnding::Finite | TraceEnding::Stuttering => None,
    };
    let ending = Ending::from_tlc(
        result.error_class.as_deref()?,
        result.violated_property.as_deref(),
        back_to,
    )?;
    match (&ending, &trace.ending) {
        (Ending::Lasso { .. }, TraceEnding::Lasso { back_to: None }) => None,
        _ => Some(ending),
    }
}

/// TLC's `-dumpTrace json` document of a golden result, with its ending.
///
/// # Errors
/// Why there is none: no counterexample, the document is unavailable
/// (issue #35) or omitted for size, or the oracle cannot write one.
pub fn tlc_document(result: &OracleResult) -> Result<TraceDoc, String> {
    let trace = result.trace.as_ref().ok_or("no counterexample")?;
    if let Some(why) = &trace.json_unavailable {
        return Err(format!("trace.json_unavailable: {why}"));
    }
    if let Some(o) = &trace.omitted {
        return Err(format!("trace omitted: {}", o.reason));
    }
    let json = trace
        .json
        .as_ref()
        .ok_or("the oracle wrote no -dumpTrace json document")?;
    let ending = ending_of(result).ok_or("the result names no ending the validator knows")?;
    let doc = TraceDoc::from_json(json)?.with_ending(ending);
    if doc.states.len() as u64 != trace.length {
        return Err(format!(
            "the document has {} states but the trace {}",
            doc.states.len(),
            trace.length
        ));
    }
    Ok(doc)
}

/// Binds a corpus model for validation.
///
/// # Errors
/// The binding error, as text.
pub fn load_model(
    corpus_root: &Path,
    model: &ModelEntry,
    module_path: &[PathBuf],
    compat_tlc: bool,
) -> Result<SpecModel, String> {
    let opts = TraceOptions {
        compat_tlc,
        module_paths: module_path.to_vec(),
        ..TraceOptions::default()
    };
    SpecModel::load(
        &corpus_root.join(&model.module),
        &corpus_root.join(&model.id),
        &opts,
    )
    .map_err(|e| e.to_string())
}

/// Validates an implementation's counterexample (`result.trace.json`, in
/// dive's format) against `spec`. `golden` is the oracle's result for the
/// same model, consulted only to confirm a deadlock (see the module
/// documentation).
#[must_use]
pub fn validate_counterexample(
    spec: &SpecModel,
    result: &OracleResult,
    golden: &OracleResult,
) -> TraceCheck {
    let Some(json) = result.trace.as_ref().and_then(|t| t.json.as_ref()) else {
        return TraceCheck::no("the result carries no counterexample document");
    };
    let doc = match TraceDoc::from_json(json) {
        Ok(d) => d,
        Err(e) => return TraceCheck::no(format!("unreadable counterexample: {e}")),
    };
    let reported = ending_of(result);
    if doc.ending.is_none() || doc.ending != reported {
        return TraceCheck::no(format!(
            "the counterexample claims {:?}, but the result reports {reported:?}",
            doc.ending
        ));
    }
    let (report, trace) = spec.read(&doc);
    match &report.validation {
        Validation::Valid => TraceCheck {
            validated: true,
            detail: "validated".to_owned(),
        },
        Validation::Unsupported(u) if u == dive_trace_deadlock() => {
            let Some(trace) = trace else {
                return TraceCheck::no("deadlock: no trace");
            };
            match golden_last_state(spec, golden) {
                Ok(last) if trace.states.last() == Some(&last) => TraceCheck {
                    validated: true,
                    detail: "validated; the deadlock is confirmed by the oracle's counterexample, which ends in the same state".to_owned(),
                },
                Ok(_) => TraceCheck::no(
                    "deadlock: every other check passed, but the last state differs from the oracle's deadlocked state",
                ),
                Err(e) => TraceCheck::no(format!(
                    "deadlock: every other check passed, but the oracle cannot confirm it: {e}"
                )),
            }
        }
        Validation::Unsupported(u) => TraceCheck::no(format!("unsupported: {u}")),
        Validation::Invalid(f) => TraceCheck::no(format!(
            "invalid at state {} ({}): {}",
            f.state,
            f.check.name(),
            f.detail
        )),
    }
}

fn dive_trace_deadlock() -> &'static str {
    dive_api::trace::DEADLOCK_UNSUPPORTED
}

/// The last state of the golden's own deadlock counterexample, read
/// against `spec` (every check but the deadlock itself must pass).
fn golden_last_state(
    spec: &SpecModel,
    golden: &OracleResult,
) -> Result<dive_api::trace::State, String> {
    if ending_of(golden) != Some(Ending::Deadlock) {
        return Err("the golden is not a deadlock".to_owned());
    }
    let doc = tlc_document(golden)?;
    let (report, trace) = spec.read(&doc);
    match (&report.validation, trace) {
        (Validation::Unsupported(u), Some(t)) if u == dive_trace_deadlock() => t
            .states
            .last()
            .cloned()
            .ok_or_else(|| "empty trace".to_owned()),
        (v, _) => Err(format!(
            "the oracle's counterexample does not read: {}",
            v.kind()
        )),
    }
}

/// The controls on one golden counterexample.
#[derive(Debug, Clone)]
pub enum ControlOutcome {
    /// The controls ran.
    Ran(Controls),
    /// The golden has no usable document (with the reason).
    Skipped(String),
    /// The model could not be bound.
    Error(String),
}

/// Runs the validator's controls on a golden counterexample under
/// `--compat=tlc`.
#[must_use]
pub fn golden_controls(
    corpus_root: &Path,
    model: &ModelEntry,
    module_path: &[PathBuf],
    golden: &OracleResult,
) -> ControlOutcome {
    let doc = match tlc_document(golden) {
        Ok(d) => d,
        Err(why) => return ControlOutcome::Skipped(why),
    };
    match load_model(corpus_root, model, module_path, true) {
        Ok(spec) => ControlOutcome::Ran(spec.controls(&doc)),
        Err(e) => ControlOutcome::Error(e),
    }
}

/// One golden's row in `trace-controls`.
#[derive(Debug, Clone)]
pub struct ControlRow {
    /// `corpus:model`.
    pub id: String,
    /// The outcome.
    pub outcome: ControlOutcome,
}

/// The result of `dive-conformance trace-controls`.
#[derive(Debug, Clone, Default)]
pub struct ControlsSummary {
    /// One row per golden with a counterexample.
    pub rows: Vec<ControlRow>,
}

impl ControlsSummary {
    /// Findings: an invalid positive control, a mutant that is not
    /// rejected, or a model the validator cannot bind. A positive control
    /// the validator refuses by name (liveness) is listed, not a finding.
    #[must_use]
    pub fn findings(&self) -> Vec<String> {
        let mut out = Vec::new();
        for r in &self.rows {
            match &r.outcome {
                ControlOutcome::Ran(c) => {
                    if matches!(&c.positive.validation, Validation::Unsupported(_))
                        && !c.positive_ok()
                    {
                        // Refused by name: only a mutant that validates is
                        // a finding, since nothing can be rejected past the
                        // refusal.
                        for (name, v) in &c.mutants {
                            if v.is_valid() {
                                out.push(format!("{}: mutant {name} validates", r.id));
                            }
                        }
                    } else {
                        out.extend(c.failures().into_iter().map(|f| format!("{}: {f}", r.id)));
                    }
                }
                ControlOutcome::Error(e) => out.push(format!("{}: {e}", r.id)),
                ControlOutcome::Skipped(_) => {}
            }
        }
        out
    }

    /// A Markdown table and totals.
    #[must_use]
    pub fn render(&self) -> String {
        use std::fmt::Write as _;
        let mut s = String::from(
            "| Golden | Positive control | Mutants rejected | Notes |\n|---|---|---:|---|\n",
        );
        let mut t = Totals::default();
        for r in &self.rows {
            let (pos, muts, note) = match &r.outcome {
                ControlOutcome::Ran(c) => {
                    let pos = match &c.positive.validation {
                        Validation::Valid => {
                            t.valid += 1;
                            "valid".to_owned()
                        }
                        Validation::Unsupported(u) if u == dive_trace_deadlock() => {
                            t.deadlock += 1;
                            "deadlock refused; rest valid".to_owned()
                        }
                        Validation::Unsupported(u) => {
                            t.unsupported += 1;
                            format!("unsupported: {u}")
                        }
                        Validation::Invalid(f) => format!(
                            "**invalid** at state {} ({}): {}",
                            f.state,
                            f.check.name(),
                            f.detail
                        ),
                    };
                    let ok = c
                        .mutants
                        .iter()
                        .filter(|(_, v)| matches!(v, Validation::Invalid(_)))
                        .count();
                    t.mutants += c.mutants.len();
                    t.rejected += ok;
                    (
                        pos,
                        format!("{ok}/{}", c.mutants.len()),
                        c.positive.notes.join("; "),
                    )
                }
                ControlOutcome::Skipped(why) => {
                    t.skipped += 1;
                    ("skipped".to_owned(), "-".to_owned(), why.clone())
                }
                ControlOutcome::Error(e) => ("**error**".to_owned(), "-".to_owned(), e.clone()),
            };
            let note = note.replace('|', "\\|").replace('\n', " ");
            let _ = writeln!(s, "| {} | {pos} | {muts} | {note} |", r.id);
        }
        let _ = writeln!(
            s,
            "\n{} golden counterexample(s): {} valid, {} deadlock (refused by name, every other check valid), {} unsupported, {} skipped; {}/{} mutants rejected; {} finding(s)",
            self.rows.len(),
            t.valid,
            t.deadlock,
            t.unsupported,
            t.skipped,
            t.rejected,
            t.mutants,
            self.findings().len()
        );
        s
    }
}

#[derive(Default)]
struct Totals {
    valid: usize,
    deadlock: usize,
    unsupported: usize,
    skipped: usize,
    mutants: usize,
    rejected: usize,
}

/// Runs the controls over every golden counterexample of the selected
/// models of `corpora` (`dive-conformance trace-controls`).
///
/// # Errors
/// Harness failures: a corpus or classpath entry missing from the cache, an
/// unreadable golden.
pub fn run_controls(
    config: &crate::config::Config,
    cache: &crate::cache::Cache,
    repo_root: &Path,
    corpora: &[String],
    selection: &crate::pipeline::Selection,
    golden_oracle: &str,
) -> crate::Result<ControlsSummary> {
    let module_path: Vec<PathBuf> = crate::dive::module_search_path(config, cache, golden_oracle)?
        .into_iter()
        .map(|e| e.path)
        .collect();
    let goldens = crate::golden::golden_root(repo_root);
    let mut summary = ControlsSummary::default();
    for id in corpora {
        let corpus = crate::pipeline::load_corpus(config, cache, id)?;
        for m in selection.models(&corpus)? {
            if m.out_of_scope.is_some() {
                continue;
            }
            let path = crate::golden::model_golden_path(&goldens, golden_oracle, &corpus.id, &m.id);
            if !path.exists() {
                continue;
            }
            let g = crate::golden::read_model_golden(&path)?;
            if g.result.trace.is_none() {
                continue;
            }
            summary.rows.push(ControlRow {
                id: format!("{}:{}", corpus.id, m.id),
                outcome: golden_controls(&corpus.root, m, &module_path, &g.result),
            });
        }
    }
    Ok(summary)
}
