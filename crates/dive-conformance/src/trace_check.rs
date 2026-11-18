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
//! **Deadlock.** The core evaluator's `ENABLED` of the next-state relation
//! decides deadlock endings (ADR 0103). When it cannot decide, the harness
//! requires every other check to pass and the final state to equal the
//! golden's deadlocked state. Refusal never proves absence.

use std::path::{Path, PathBuf};

use dive_api::trace::{
    ControlMode, Controls, Ending, SpecModel, TraceDoc, TraceOptions, Validation,
};

use crate::corpus::{ModelEntry, ModelMode};
use crate::tlc::result::{OracleResult, TraceEnding};

/// The validator's answer for one counterexample.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TraceCheck {
    /// Whether the counterexample validated.
    pub validated: bool,
    /// Whether it was shown wrong (invalid, or contradicted by the oracle),
    /// rather than left unconfirmed (refused by name, or no document).
    pub refuted: bool,
    /// What the validator said.
    pub detail: String,
    // Only fresh independent validation below records this exact reported list.
    // Generic `yes` hooks cannot grant word-relative property comparison.
    liveness_property: Option<String>,
}

impl TraceCheck {
    /// Validated.
    #[must_use]
    pub fn yes(detail: impl Into<String>) -> Self {
        Self {
            validated: true,
            refuted: false,
            detail: detail.into(),
            liveness_property: None,
        }
    }

    /// Not confirmed: refused by name, or nothing to validate.
    #[must_use]
    pub fn no(detail: impl Into<String>) -> Self {
        Self {
            validated: false,
            refuted: false,
            detail: detail.into(),
            liveness_property: None,
        }
    }

    /// Shown wrong.
    #[must_use]
    pub fn refuted(detail: impl Into<String>) -> Self {
        Self {
            validated: false,
            refuted: true,
            detail: detail.into(),
            liveness_property: None,
        }
    }

    pub(crate) fn confirms_liveness_property(&self, result: &OracleResult) -> bool {
        self.validated
            && result.verdict == crate::tlc::result::Verdict::LivenessViolation
            && self.liveness_property.is_some()
            && self.liveness_property == result.violated_property
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

// Same pinned MP list formatting as the report; no name is inferred from text.
pub(crate) fn property_names(names: &[String]) -> String {
    match names.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, [first])) => format!("{first} and {last}"),
        Some((last, rest)) => format!("{}, and {last}", rest.join(", ")),
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
        // The goldens' runs: `-seed 0 -fp 0`.
        run: Some((0, 0)),
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

// Checks report/display consistency only. Fresh independent validation below
// must still establish the word and its complete property attribution.
fn reported_liveness_property(
    result: &OracleResult,
    doc: &TraceDoc,
) -> Result<Option<String>, TraceCheck> {
    if result.verdict != crate::tlc::result::Verdict::LivenessViolation {
        return Ok(None);
    }
    if doc.properties.is_empty() {
        return Err(TraceCheck::refuted(
            "a liveness report must name the properties false on its word",
        ));
    }
    let names = property_names(&doc.properties);
    if result.violated_property.as_deref() != Some(names.as_str()) {
        return Err(TraceCheck::refuted(format!(
            "reported liveness names {:?} differ from the word's names {names:?}",
            result.violated_property
        )));
    }
    Ok(Some(names))
}

// Header consistency is necessary; the candidate and both exception identities
// still require actual source-bound execution in the validator below.
fn candidate_error_header(ec: &str, deciding: Option<u32>) -> Option<Ending> {
    match deciding {
        Some(2103) => Some(Ending::Error {
            ec: Some("TLC_NESTED_EXPRESSION".into()),
        }),
        Some(2132) => Some(Ending::Assertion),
        Some(2179) if ec == "TLC_MODULE_DIVISION_BY_ZERO" => Some(Ending::Error {
            ec: Some(ec.into()),
        }),
        Some(2178) if ec == "TLC_MODULE_OVERFLOW" => Some(Ending::Error {
            ec: Some(ec.into()),
        }),
        _ => None,
    }
}

fn source_error_header(ec: &str, result: &OracleResult, reported: Option<&Ending>) -> bool {
    ec == "GENERAL"
        && result.verdict == crate::tlc::result::Verdict::OtherError
        && result.verdict_code == Some(1000)
        && result.exit_code == Some(255)
        && result.error_class.as_deref() == Some("GENERAL")
        && result.violated_property.is_none()
        && matches!(reported, Some(Ending::Error { ec: Some(code) }) if code == "GENERAL")
}

fn invariant_header(
    spec: &SpecModel,
    doc: &TraceDoc,
    result: &OracleResult,
    reported: Option<&Ending>,
    convention: dive_api::ExitConvention,
) -> bool {
    let Ok((proof, Some(class))) = spec.invariant_error_class(doc, None) else {
        return false;
    };
    let Ok(code) = i32::try_from(class.deciding_code()) else {
        return false;
    };
    proof.validation.is_valid()
        && crate::dive::successor_invariant::canonical_result(
            result,
            class.deciding_code(),
            convention,
        )
        && crate::tlc::codes::ec_name(code).is_some_and(|name| {
            reported
                == Some(&Ending::Error {
                    ec: Some(name.into()),
                })
        })
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
    validate_counterexample_with_exit_convention(
        spec,
        result,
        golden,
        dive_api::ExitConvention::Tlc,
    )
}

/// Validates a word and its error report under an explicitly selected process
/// status convention; serialized report metadata cannot select that convention.
///
/// ```no_run
/// # fn native(spec: &dive_api::trace::SpecModel,
/// # result: &dive_conformance::tlc::result::OracleResult) {
/// let proof = dive_conformance::trace_check::validate_counterexample_with_exit_convention(
///     spec, result, result, dive_api::ExitConvention::Native,
/// );
/// # let _ = proof; }
/// ```
#[must_use]
#[allow(
    clippy::too_many_lines,
    reason = "one admission keeps report identity, word replay and deadlock qualification ordered"
)]
pub fn validate_counterexample_with_exit_convention(
    spec: &SpecModel,
    result: &OracleResult,
    golden: &OracleResult,
    convention: dive_api::ExitConvention,
) -> TraceCheck {
    let Some(json) = result.trace.as_ref().and_then(|t| t.json.as_ref()) else {
        return TraceCheck::no("the result carries no counterexample document");
    };
    let doc = match TraceDoc::from_json(json) {
        Ok(d) => d,
        Err(e) => return TraceCheck::no(format!("unreadable counterexample: {e}")),
    };
    let attributed = match reported_liveness_property(result, &doc) {
        Ok(names) => names,
        Err(refuted) => return refuted,
    };
    let reported = ending_of(result);
    let matching = match &doc.ending {
        // This claim names the underlying error and the invariant that
        // raised it; the report names TLC's site wrapper. Independent
        // validation below must establish the exact named claim.
        Some(Ending::InvariantError { .. }) => {
            invariant_header(spec, &doc, result, reported.as_ref(), convention)
        }
        Some(Ending::NextStateError { ec, endpoint }) => {
            let expected = match endpoint.deciding_code() {
                Some(2147) => Some(Ending::Error {
                    ec: Some("TLC_FINGERPRINT_EXCEPTION".into()),
                }),
                Some(2103) => Some(Ending::Error {
                    ec: Some("TLC_NESTED_EXPRESSION".into()),
                }),
                Some(2132) => Some(Ending::Assertion),
                _ if ec == "TLC_VALUE_ASSERT_FAILED" => Some(Ending::Assertion),
                _ => Some(Ending::Error {
                    ec: Some(ec.clone()),
                }),
            };
            reported == expected
        }
        Some(Ending::ViewSourceError { ec, endpoint }) => {
            let expected = candidate_error_header(ec, Some(endpoint.deciding_code()));
            reported == expected
                && expected.is_some()
                && crate::dive::successor_invariant::canonical_result(
                    result,
                    endpoint.deciding_code(),
                    convention,
                )
        }
        Some(Ending::CandidateStateError { ec, endpoint }) => {
            let expected = candidate_error_header(ec, endpoint.deciding_code());
            reported == expected && expected.is_some()
        }
        Some(Ending::TemporalStateError { ec, .. }) => {
            reported
                == Some(Ending::Error {
                    ec: Some(ec.clone()),
                })
        }
        Some(Ending::TemporalSourceError { ec, .. }) => {
            source_error_header(ec, result, reported.as_ref())
        }
        Some(Ending::PartialNext { .. }) => {
            reported
                == Some(Ending::Error {
                    ec: Some("TLC_STATE_NOT_COMPLETELY_SPECIFIED_NEXT".to_owned()),
                })
        }
        _ => doc.ending.is_some() && doc.ending == reported,
    };
    if !matching {
        return TraceCheck::refuted(format!(
            "the counterexample claims {:?}, but the result reports {reported:?}",
            doc.ending
        ));
    }
    let (report, trace) = match spec.try_read(&doc) {
        Ok(read) => read,
        Err(error) => {
            return TraceCheck::no(format!("independent validation did not complete: {error}"));
        }
    };
    match &report.validation {
        Validation::Valid => {
            let mut check = TraceCheck::yes("validated");
            check.liveness_property = attributed;
            check
        }
        Validation::Unsupported(u) if u == dive_trace_deadlock() => {
            let Some(trace) = trace else {
                return TraceCheck::no("deadlock: no trace");
            };
            match golden_last_state(spec, golden) {
                Ok(last) if trace.states.last() == Some(&last) => TraceCheck::yes(
                    "validated; the deadlock is confirmed by the oracle's counterexample, which ends in the same state",
                ),
                Ok(_) => TraceCheck::refuted(
                    "deadlock: every other check passed, but the last state differs from the oracle's deadlocked state",
                ),
                Err(e) => TraceCheck::no(format!(
                    "deadlock: every other check passed, but the oracle cannot confirm it: {e}"
                )),
            }
        }
        Validation::Unsupported(u) => TraceCheck::no(format!("unsupported: {u}")),
        Validation::Invalid(f) => TraceCheck::refuted(format!(
            "invalid at state {} ({}): {}",
            f.state,
            f.check.name(),
            f.detail
        )),
        other => TraceCheck::no(format!("independent validation returned {}", other.kind())),
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
        (Validation::Valid, Some(t)) => t
            .states
            .last()
            .cloned()
            .ok_or_else(|| "empty trace".to_owned()),
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
/// `--compat=tlc`. Only single-worker exhaustive finite counterexamples
/// carry the shortest-BFS contract; all other modes and liveness endings
/// use controls without a shortest-path premise.
#[must_use]
pub fn golden_controls(
    corpus_root: &Path,
    model: &ModelEntry,
    module_path: &[PathBuf],
    golden: &OracleResult,
    workers: u32,
) -> ControlOutcome {
    let doc = match tlc_document(golden) {
        Ok(d) => d,
        Err(why) => return ControlOutcome::Skipped(why),
    };
    let mode = control_mode(model.mode, workers, doc.ending.as_ref());
    match load_model(corpus_root, model, module_path, true) {
        Ok(spec) => ControlOutcome::Ran(spec.controls_with_mode(&doc, mode)),
        Err(e) => ControlOutcome::Error(e),
    }
}

fn control_mode(mode: ModelMode, workers: u32, ending: Option<&Ending>) -> ControlMode {
    if mode == ModelMode::Exhaustive
        && workers == 1
        && matches!(
            ending,
            Some(
                Ending::Invariant(_)
                    | Ending::InvariantError { .. }
                    | Ending::Property(_)
                    | Ending::Deadlock
                    | Ending::Assertion
                    | Ending::Error { .. }
            )
        )
    {
        ControlMode::ShortestBfs
    } else {
        ControlMode::Counterexample
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
                        other => format!("**{}**", other.kind()),
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
                outcome: golden_controls(&corpus.root, m, &module_path, &g.result, g.workers),
            });
        }
    }
    Ok(summary)
}

#[cfg(test)]
mod control_mode_tests {
    use super::control_mode;
    use crate::corpus::ModelMode;
    use dive_api::trace::{ControlMode, Ending};

    #[test]
    fn shortest_controls_require_single_worker_bfs_and_a_finite_claim() {
        let finite = [
            Ending::Invariant("P".to_owned()),
            Ending::InvariantError {
                name: "P".to_owned(),
                ec: "TLC_VALUE_ASSERT_FAILED".to_owned(),
            },
            Ending::Property("P".to_owned()),
            Ending::Deadlock,
            Ending::Assertion,
            Ending::Error { ec: None },
        ];
        for ending in &finite {
            assert_eq!(
                control_mode(ModelMode::Exhaustive, 1, Some(ending)),
                ControlMode::ShortestBfs
            );
            for workers in [0, 2, 4] {
                assert_eq!(
                    control_mode(ModelMode::Exhaustive, workers, Some(ending)),
                    ControlMode::Counterexample
                );
            }
            for mode in [
                ModelMode::Simulate { traces: 100 },
                ModelMode::Generate,
                ModelMode::Symbolic,
            ] {
                assert_eq!(
                    control_mode(mode, 1, Some(ending)),
                    ControlMode::Counterexample
                );
            }
        }
        for ending in [
            Ending::Behavior,
            Ending::Lasso { back_to: 0 },
            Ending::Stuttering,
        ] {
            assert_eq!(
                control_mode(ModelMode::Exhaustive, 1, Some(&ending)),
                ControlMode::Counterexample
            );
        }
        assert_eq!(
            control_mode(ModelMode::Exhaustive, 1, None),
            ControlMode::Counterexample
        );
    }
}
