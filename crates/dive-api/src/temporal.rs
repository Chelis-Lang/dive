//! Bounded final temporal checking for Rust callers.
//!
//! This API checks the supported pure temporal subset and independently validates
//! every periodic counterexample before returning it as a violation. It binds a
//! fresh model for each run and for validation. It selects a deterministic short
//! lasso, including with TLC value semantics. Eligible ordinary [`crate::check_file`]
//! requests use final checking in the TLC profile and deterministic completed-level
//! checks in the dive profile. Explicit final/short selection remains unchanged;
//! neither schedule reproduces TLC's periodic timing or exact lasso selection.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dive_check::{CheckOptions, CheckResult, capture};
use dive_config::ModelConfig;
use dive_ir::Ir;
use dive_liveness::analyze::periodic::{Checks, PeriodicAnalysis, PrefixCandidate, Schedule};
use dive_liveness::analyze::{Analysis, AnalysisError, Candidate, Limits};
use dive_liveness::product::AcceptancePolicy;
use dive_model::{BindOptions, Model};
use dive_trace::{Ending, Report as ValidationReport, Trace, TraceDoc, Validation};

/// The outcome of a bounded temporal run, distinguishing real prefixes.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// Every planned property holds on the fully captured graph, subject to
    /// the exploration's reported fingerprint collision qualifications.
    Holds(Box<capture::Complete>),
    /// A concrete periodic word falsifies the named properties and passes
    /// the independent validator. TLC's exact lasso selection is not claimed.
    Violation(Box<ValidatedCounterexample>),
    /// Independently validated word on a real incomplete capture prefix.
    PrefixViolation(Box<ValidatedPrefix>),
    /// The ordinary safety result when no properties are configured, when
    /// configured properties bind entirely to safety roles on the supported
    /// default-memory route, or a safety stop before temporal checking completes.
    /// This preserves its verdict, statistics, output and counterexample;
    /// unsupported features and resource limits are never success.
    Safety(Box<CheckResult>),
}

/// An independently validated periodic counterexample. The fields are private
/// so callers cannot construct this evidence from an unvalidated candidate.
#[derive(Clone, Debug)]
pub struct ValidatedCounterexample {
    candidate: Candidate,
    document: TraceDoc,
    validation: ValidationReport,
}
impl ValidatedCounterexample {
    /// The short candidate and its complete exploration evidence.
    #[must_use]
    pub fn candidate(&self) -> &Candidate {
        &self.candidate
    }
    /// Independently admitted JSON trace data and the complete false-property set.
    /// Tagged values retain the trace format's documented representation limits.
    #[must_use]
    pub fn document(&self) -> &TraceDoc {
        &self.document
    }
    /// The independent validator's report, always [`Validation::Valid`].
    #[must_use]
    pub fn validation(&self) -> &ValidationReport {
        &self.validation
    }
}

/// A prefix word admitted by fresh independent typed and serialized validation.
/// It cannot supply complete exploration evidence or a holding verdict.
#[derive(Clone, Debug)]
pub struct ValidatedPrefix {
    candidate: PrefixCandidate,
    document: TraceDoc,
    validation: ValidationReport,
}
impl ValidatedPrefix {
    /// Original candidate and actual committed-prefix capture evidence.
    #[must_use]
    pub fn candidate(&self) -> &PrefixCandidate {
        &self.candidate
    }
    /// Independently validated serialized word and false-property names.
    #[must_use]
    pub fn document(&self) -> &TraceDoc {
        &self.document
    }
    /// The actual independent serialized validation report, always valid.
    #[must_use]
    pub fn validation(&self) -> &ValidationReport {
        &self.validation
    }
}

/// A temporal result with total binding, exploration and validation time.
#[derive(Clone, Debug)]
pub struct Report {
    outcome: Outcome,
    options: CheckOptions,
    elapsed: Duration,
    periodic: Option<Checks>,
}
impl Report {
    /// The temporal outcome or the complete safety stop.
    #[must_use]
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }
    /// Run settings, including the seed and profile, even on a safety stop.
    #[must_use]
    pub fn options(&self) -> &CheckOptions {
        &self.options
    }
    /// Actual deterministic checks, absent for the final-only schedule.
    #[must_use]
    pub const fn periodic(&self) -> Option<Checks> {
        self.periodic
    }
    /// Actual schedule used, independent of the value-semantics profile.
    #[must_use]
    pub const fn lncheck(&self) -> &'static str {
        if self.periodic.is_some() {
            "deterministic"
        } else {
            "final"
        }
    }
    /// Total wall-clock time, including independent validation.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

/// An explicit refusal, evaluation error, resource bound or internal inconsistency. No error
/// variant is evidence of a successful model check.
#[derive(Clone, Debug)]
pub enum Error {
    /// Binding failed, preserving diagnostics and startup output.
    Binding(Box<dive_model::Failure>),
    /// Planning, product construction, atom evaluation or an explicit bound.
    Analysis(AnalysisError),
    /// The independent validator cannot decide the candidate, or refutes it.
    /// Invalid candidates indicate an internal error and are never violations.
    Validation {
        /// The candidate whose claim could not be certified.
        candidate: Box<Candidate>,
        /// Independent validation details.
        report: ValidationReport,
    },
    /// Independent validation refused or refuted an incomplete-prefix word.
    PrefixValidation {
        /// Actual prefix candidate, never a completed graph.
        candidate: Box<PrefixCandidate>,
        /// Actual independent validator report.
        report: ValidationReport,
    },
    /// A value could not be represented in the tagged trace document.
    Document(String),
    /// The evaluation thread failed to start or panicked.
    Thread(String),
}
impl Error {
    /// Stable diagnostic code. An invalid independent result has a separate
    /// internal-error code from an unsupported validation result.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Binding(_) => "DIVE-A0030",
            Self::Analysis(e) => e.code(),
            Self::Validation { report, .. } | Self::PrefixValidation { report, .. }
                if matches!(report.validation, Validation::Invalid(_)) =>
            {
                "DIVE-A0031"
            }
            Self::Validation { .. } | Self::PrefixValidation { .. } => "DIVE-A0032",
            Self::Document(_) => "DIVE-A0033",
            Self::Thread(_) => "DIVE-A0034",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Binding(e) => e.fmt(f),
            Self::Analysis(e) => e.fmt(f),
            Self::Validation { report, .. } | Self::PrefixValidation { report, .. } => write!(
                f,
                "{}: temporal candidate did not pass independent validation: {:?}",
                self.code(),
                report.validation
            ),
            Self::Document(e) => write!(f, "{}: temporal trace document: {e}", self.code()),
            Self::Thread(e) => write!(f, "{}: temporal evaluation thread: {e}", self.code()),
        }
    }
}
impl std::error::Error for Error {}

/// Stage reached by a temporal run that produced no verdict.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stage {
    /// Initial model binding; no capture started.
    Binding,
    /// A known stage of final temporal analysis.
    Analysis(dive_liveness::analyze::Stage),
    /// Independent validation, including binding a fresh validation model.
    Validation,
    /// Conversion to the tagged trace document.
    Document,
    /// A thread failure prevented recovery of execution evidence.
    Unknown,
}
impl Stage {
    /// Stable execution-stage name for structured reports.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Binding => "binding",
            Self::Analysis(stage) => stage.name(),
            Self::Validation => "validation",
            Self::Document => "document",
            Self::Unknown => "unknown",
        }
    }
}

/// A temporal failure with retained execution facts, never a verdict.
/// Existing [`check_final`] callers keep their original [`Error`] interface;
/// [`check_final_with_evidence`] exposes these additional facts.
#[derive(Clone, Debug)]
pub struct Failure {
    error: Box<Error>,
    stage: Stage,
    exploration: Option<Box<dive_liveness::analyze::ExplorationEvidence>>,
    elapsed: Duration,
    action_evaluation: Option<Box<dive_liveness::analyze::ActionEvaluation>>,
    incomplete: bool,
    periodic: Option<Box<Checks>>,
}
impl Failure {
    fn before(error: Error, stage: Stage) -> Self {
        Self {
            error: Box::new(error),
            stage,
            exploration: None,
            elapsed: Duration::ZERO,
            action_evaluation: None,
            incomplete: false,
            periodic: None,
        }
    }
    fn after(error: Error, stage: Stage, capture: &capture::Complete) -> Self {
        Self {
            error: Box::new(error),
            stage,
            exploration: Some(Box::new(capture.into())),
            elapsed: Duration::ZERO,
            action_evaluation: None,
            incomplete: false,
            periodic: None,
        }
    }
    /// The actual typed active action-label error, when one occurred.
    /// This does not claim TLC's error ordering across multiple properties.
    #[must_use]
    pub fn action_evaluation(&self) -> Option<&dive_liveness::analyze::ActionEvaluation> {
        self.action_evaluation.as_deref()
    }
    /// Original diagnostic, preserving its code and classification.
    #[must_use]
    pub fn error(&self) -> &Error {
        &self.error
    }
    /// Stage reached before the failure.
    #[must_use]
    pub const fn stage(&self) -> Stage {
        self.stage
    }
    /// Actual capture facts, when available. Consult [`Self::capture_completed`]
    /// to distinguish a committed prefix from complete exploration.
    #[must_use]
    pub fn exploration(&self) -> Option<&dive_liveness::analyze::ExplorationEvidence> {
        self.exploration.as_deref()
    }
    /// Total time including binding, exploration and any validation work.
    #[must_use]
    pub const fn elapsed(&self) -> Duration {
        self.elapsed
    }
    /// Whether capture completed. Unknown thread failures yield `None`, not
    /// a claim that no work ran.
    #[must_use]
    pub fn capture_completed(&self) -> Option<bool> {
        if self.incomplete {
            Some(false)
        } else if self.exploration.is_some() {
            Some(true)
        } else if matches!(
            self.stage,
            Stage::Binding | Stage::Analysis(dive_liveness::analyze::Stage::Planning)
        ) {
            Some(false)
        } else {
            None
        }
    }
    /// Actual deterministic scheduling work before this failure, if selected.
    #[must_use]
    pub fn periodic(&self) -> Option<Checks> {
        self.periodic.as_deref().copied()
    }
    /// Actual execution width where known. Pre-search refusals prove zero;
    /// unobserved thread failures do not.
    #[must_use]
    pub fn workers(&self, requested: usize) -> Option<dive_check::Workers> {
        if let Some(evidence) = &self.exploration {
            Some(evidence.workers.clone())
        } else if self.capture_completed() == Some(false) {
            Some(dive_check::Workers {
                requested,
                used: 0,
                sequential: false,
                reason: Some("no search ran".into()),
            })
        } else {
            None
        }
    }
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for Failure {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        Some(&*self.error)
    }
}

fn bind(ir: &Arc<Ir>, config: &ModelConfig, options: &CheckOptions) -> Result<Model, Error> {
    if options.seen_disk.is_some() {
        return Err(Error::Analysis(AnalysisError::Unsupported(
            "disk seen-state retention with temporal capture is not implemented",
        )));
    }
    let mode = if options.profile == dive_eval::Profile::Tlc {
        dive_stdlib::random::RandomMode::Tlc
    } else {
        dive_stdlib::random::RandomMode::Dive
    };
    let host = Arc::new(dive_stdlib::host::RunHost::new(
        dive_stdlib::random::Streams::new(options.seed, mode),
        None,
    ));
    dive_model::bind(
        ir.clone(),
        config,
        &BindOptions {
            eval: options.eval,
            no_deadlock: options.no_deadlock,
            host: Some(host),
        },
    )
    .map_err(Error::Binding)
}

fn run(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
) -> Result<Outcome, Failure> {
    let model = match dive_check::final_roles::dispatch(ir, config, options) {
        Some(Ok(dive_check::final_roles::Dispatch::Safety(result))) => {
            return Ok(Outcome::Safety(result));
        }
        Some(Ok(dive_check::final_roles::Dispatch::Temporal(model))) => *model,
        Some(Err(error)) => return Err(Failure::before(Error::Binding(error), Stage::Binding)),
        None => {
            bind(ir, config, options).map_err(|error| Failure::before(error, Stage::Binding))?
        }
    };
    run_bound(ir, config, options, limits, &model, policy)
}

fn run_bound(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    limits: Limits,
    model: &Model,
    policy: AcceptancePolicy,
) -> Result<Outcome, Failure> {
    finish_final(
        ir,
        config,
        options,
        dive_liveness::analyze::analyze_final_with_policy(model, options, limits, policy),
    )
}

fn finish_final(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    analysis: Result<Analysis, dive_liveness::analyze::DetailedFailure>,
) -> Result<Outcome, Failure> {
    match analysis {
        Ok(Analysis::Holds(complete)) => Ok(Outcome::Holds(Box::new(complete))),
        Ok(Analysis::Candidate(candidate)) => {
            // Rebind separately: validation cannot inspect the checker's graph
            // or depend on its mutable evaluator/host state.
            let validation_model = bind(ir, config, options).map_err(|error| {
                Failure::after(error, Stage::Validation, &candidate.exploration)
            })?;
            let trace = Trace {
                states: candidate.states.clone(),
                // Labels are optional; the validator checks the relation itself.
                // Closing/implicit stuttering semantics come from the ending.
                labels: vec![None; candidate.states.len()],
                ending: Ending::Lasso {
                    back_to: candidate.back_to,
                },
                length: Some(candidate.states.len()),
                properties: candidate.properties.clone(),
            };
            let report = dive_trace::validate(&validation_model, &trace);
            if report.validation != Validation::Valid {
                return Err(final_validation_failure(candidate, report));
            }
            let document = trace.to_doc(&validation_model).map_err(|error| {
                Failure::after(
                    Error::Document(error),
                    Stage::Document,
                    &candidate.exploration,
                )
            })?;
            // Validate the serialized form too; a conversion bug must not turn
            // a certified in-memory word into an incorrect user-facing trace.
            let report =
                dive_trace::validate_doc(&validation_model, &document, &config.model_values).0;
            if report.validation != Validation::Valid {
                return Err(final_validation_failure(candidate, report));
            }
            Ok(Outcome::Violation(Box::new(ValidatedCounterexample {
                candidate,
                document,
                validation: report,
            })))
        }
        Err(detailed) => {
            let (failure, action_evaluation) = detailed.into_parts();
            if let AnalysisError::Exploration(result) = failure.error {
                Ok(Outcome::Safety(result))
            } else {
                Err(Failure {
                    error: Box::new(Error::Analysis(failure.error)),
                    stage: Stage::Analysis(failure.stage),
                    exploration: failure.exploration,
                    elapsed: Duration::ZERO,
                    action_evaluation,
                    incomplete: false,
                    periodic: None,
                })
            }
        }
    }
}

fn final_validation_failure(candidate: Candidate, report: ValidationReport) -> Failure {
    let exploration = Some(Box::new((&candidate.exploration).into()));
    Failure {
        error: Box::new(Error::Validation {
            candidate: Box::new(candidate),
            report,
        }),
        stage: Stage::Validation,
        exploration,
        elapsed: Duration::ZERO,
        action_evaluation: None,
        incomplete: false,
        periodic: None,
    }
}

fn prefix_failure(error: Error, stage: Stage, prefix: &capture::Prefix) -> Failure {
    Failure {
        error: Box::new(error),
        stage,
        exploration: Some(Box::new(prefix.into())),
        elapsed: Duration::ZERO,
        action_evaluation: None,
        incomplete: true,
        periodic: None,
    }
}

fn prefix_validation_failure(candidate: PrefixCandidate, report: ValidationReport) -> Failure {
    let exploration = Some(Box::new((&candidate.exploration).into()));
    Failure {
        error: Box::new(Error::PrefixValidation {
            candidate: Box::new(candidate),
            report,
        }),
        stage: Stage::Validation,
        exploration,
        elapsed: Duration::ZERO,
        action_evaluation: None,
        incomplete: true,
        periodic: None,
    }
}

fn validate_prefix(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    candidate: PrefixCandidate,
) -> Result<Outcome, Failure> {
    // Same independent checks as final candidates, with distinct prefix evidence.
    let model = bind(ir, config, options)
        .map_err(|error| prefix_failure(error, Stage::Validation, &candidate.exploration))?;
    let trace = Trace {
        states: candidate.states.clone(),
        labels: vec![None; candidate.states.len()],
        ending: Ending::Lasso {
            back_to: candidate.back_to,
        },
        length: Some(candidate.states.len()),
        properties: candidate.properties.clone(),
    };
    let report = dive_trace::validate(&model, &trace);
    if report.validation != Validation::Valid {
        return Err(prefix_validation_failure(candidate, report));
    }
    let document = trace.to_doc(&model).map_err(|error| {
        prefix_failure(
            Error::Document(error),
            Stage::Document,
            &candidate.exploration,
        )
    })?;
    let report = dive_trace::validate_doc(&model, &document, &config.model_values).0;
    if report.validation != Validation::Valid {
        return Err(prefix_validation_failure(candidate, report));
    }
    Ok(Outcome::PrefixViolation(Box::new(ValidatedPrefix {
        candidate,
        document,
        validation: report,
    })))
}

fn run_periodic(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    model: &Model,
    limits: Limits,
    policy: AcceptancePolicy,
    schedule: Schedule,
) -> Result<(Outcome, Checks), Failure> {
    let (analysis, checks) = match dive_liveness::analyze::periodic::analyze_periodic(
        model, options, limits, policy, schedule,
    ) {
        Ok(result) => result,
        Err(failure) => {
            let (detail, action_evaluation) = (*failure.failure).into_parts();
            if let AnalysisError::Exploration(result) = detail.error {
                return Ok((Outcome::Safety(result), failure.checks));
            }
            return Err(Failure {
                error: Box::new(Error::Analysis(detail.error)),
                stage: Stage::Analysis(detail.stage),
                exploration: detail.exploration.or(failure.prefix),
                elapsed: Duration::ZERO,
                action_evaluation,
                incomplete: failure.capture_completed == Some(false),
                periodic: Some(Box::new(failure.checks)),
            });
        }
    };
    let outcome = match analysis {
        PeriodicAnalysis::Final(analysis) => finish_final(ir, config, options, Ok(analysis)),
        PeriodicAnalysis::Prefix(candidate) => validate_prefix(ir, config, options, candidate),
    }
    .map_err(|mut failure| {
        failure.periodic = Some(Box::new(checks));
        failure
    })?;
    Ok((outcome, checks))
}

// Ordinary requests use only the accepted default-memory role dispatch. Keep
// its original model/host through analysis, and its complete safety results.
// This is private source/configuration dispatch, never a prebound caller API.
pub(crate) fn check_ordinary_final(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    schedule: Schedule,
) -> Option<Result<Report, Failure>> {
    if config.properties.is_empty() {
        return None;
    }
    let started = Instant::now();
    let outcome = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, || {
                dive_check::final_roles::dispatch_ordinary(ir, config, options).map(|bound| {
                    match bound {
                        dive_check::final_roles::Dispatch::Safety(result) => {
                            Ok((Outcome::Safety(result), None))
                        }
                        dive_check::final_roles::Dispatch::Temporal(model) => {
                            if options.profile == dive_eval::Profile::Dive {
                                run_periodic(
                                    ir,
                                    config,
                                    options,
                                    &model,
                                    Limits::default(),
                                    AcceptancePolicy::compact_default(),
                                    schedule,
                                )
                                .map(|(outcome, checks)| (outcome, Some(checks)))
                            } else {
                                run_bound(
                                    ir,
                                    config,
                                    options,
                                    Limits::default(),
                                    &model,
                                    AcceptancePolicy::compact_default(),
                                )
                                .map(|outcome| (outcome, None))
                            }
                        }
                    }
                })
            })
            .map_err(|error| Failure::before(Error::Thread(error.to_string()), Stage::Unknown))?
            .join()
            .map_err(|_| {
                Failure::before(
                    Error::Thread("the evaluation thread panicked".into()),
                    Stage::Unknown,
                )
            })
    });
    match outcome {
        Ok(None) => None,
        Ok(Some(result)) => Some(
            result
                .map(|(outcome, periodic)| Report {
                    outcome,
                    options: options.clone(),
                    elapsed: started.elapsed(),
                    periodic,
                })
                .map_err(|mut failure| {
                    failure.elapsed = started.elapsed();
                    failure
                }),
        ),
        Err(mut failure) => {
            failure.elapsed = started.elapsed();
            Some(Err(failure))
        }
    }
}

/// Binds and checks a lowered specification's pure temporal properties after
/// complete exploration, returning only independently validated violations.
/// The run uses [`dive_liveness::analyze::Limits`] and the supplied profile,
/// store, seed and worker request. Capture uses the ordered parallel engine
/// with bounded source waves and reports its actual pool width. Selection is
/// always `short`, including when `options`
/// requests TLC value semantics: this API does not claim byte-identical TLC
/// lassos, error timing or full TLC compatibility.
///
/// ```
/// use std::sync::Arc;
/// use dive_api::temporal::{check_final, Outcome};
/// use dive_check::CheckOptions;
/// use dive_liveness::analyze::Limits;
/// let src = "---- MODULE M ----\nVARIABLE x\nInit == x = TRUE\nNext == x' = x\nP == []<>(~x)\n====\n";
/// let analysis = dive_sema::analyze_source("M.tla", src, &dive_sema::Options::default());
/// let ir = Arc::new(dive_ir::lower(&analysis)?);
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n").into_config().unwrap();
/// let report = check_final(&ir, &cfg, &CheckOptions::dive(17), Limits::default())?;
/// let Outcome::Violation(violation) = report.outcome() else { panic!("P is false") };
/// assert_eq!(violation.document().properties, ["P"]);
/// assert_eq!(violation.validation().validation, dive_trace::Validation::Valid);
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// Explicitly unsupported temporal scopes/features, entry-count resource bounds,
/// binding/evaluation failures, failed independent validation or internal errors.
/// Safety stops are retained as [`Outcome::Safety`], never a temporal verdict.
/// With no configured `PROPERTY` entries, the ordinary safety checker runs
/// directly and its complete result is returned as [`Outcome::Safety`]. Temporal
/// analysis limits do not apply to that route; safety settings remain unchanged.
/// Configured properties also return ordinary safety results when their actual
/// bound roles contain no temporal obligations and the request meets
/// [`dive_check::final_roles::dispatch`]'s default-memory scope. Binding runs once
/// under the original startup host and retains every initial/invariant/action
/// obligation. Excluded options and genuine temporal roles keep their existing
/// analysis route; no ordinary result issues a temporal holding/capture claim.
pub fn check_final(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    limits: Limits,
) -> Result<Report, Error> {
    check_final_with_evidence(ir, config, options, limits).map_err(|failure| *failure.error)
}

/// Like [`check_final`], preserving execution-stage, worker, statistics and
/// elapsed-time evidence on failures. Capture statistics never certify a
/// temporal verdict when a later stage fails.
///
/// # Errors
/// The same failures as [`check_final`], with available execution evidence.
pub fn check_final_with_evidence(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    limits: Limits,
) -> Result<Report, Failure> {
    check_final_with_policy(ir, config, options, limits, AcceptancePolicy::Legacy)
}

/// Like [`check_final_with_evidence`], explicitly selecting compact acceptance
/// byte/work admission. Existing `check_final*` entrypoints retain legacy logical
/// cells, including caller-supplied custom limits. Buffer mode bounds only the
/// named acceptance owners; it is not a process-memory or search-work limit.
///
/// ```no_run
/// # fn example(ir: &std::sync::Arc<dive_ir::Ir>, cfg: &dive_config::ModelConfig) {
/// use dive_liveness::{analyze::Limits, product::AcceptancePolicy};
/// let _report = dive_api::temporal::check_final_with_policy(
///     ir, cfg, &dive_check::CheckOptions::tlc(7, 0), Limits::default(),
///     AcceptancePolicy::compact_default());
/// # }
/// ```
///
/// # Errors
/// The same failures as [`check_final_with_evidence`], plus named buffer byte,
/// allocation and materialization-work refusals for the selected policy.
pub fn check_final_with_policy(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
) -> Result<Report, Failure> {
    let started = Instant::now();
    if config.properties.is_empty() {
        // No temporal claim was requested. Enter the ordinary checker before
        // temporal binding so startup effects run once under its real host.
        return Ok(Report {
            outcome: Outcome::Safety(Box::new(dive_check::check(ir, config, options))),
            options: options.clone(),
            elapsed: started.elapsed(),
            periodic: None,
        });
    }
    let outcome = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(s, || run(ir, config, options, limits, policy))
            .map_err(|e| Failure::before(Error::Thread(e.to_string()), Stage::Unknown))?
            .join()
            .map_err(|_| {
                Failure::before(
                    Error::Thread("the evaluation thread panicked".into()),
                    Stage::Unknown,
                )
            })?
    })
    .map_err(|mut failure| {
        failure.elapsed = started.elapsed();
        failure
    })?;
    Ok(Report {
        outcome,
        options: options.clone(),
        elapsed: started.elapsed(),
        periodic: None,
    })
}

#[cfg(test)]
mod evidence_controls {
    use super::*;

    #[test]
    fn lost_thread_evidence_is_unknown_not_zero() {
        for stage in [
            Stage::Unknown,
            Stage::Analysis(dive_liveness::analyze::Stage::Unknown),
        ] {
            let failure = Failure::before(Error::Thread("test thread failure".into()), stage);
            assert_eq!(failure.stage().name(), "unknown");
            assert_eq!(failure.capture_completed(), None);
            assert!(failure.workers(8).is_none());
            assert!(failure.exploration().is_none());
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "prefix publication boundary assertions"
)]
mod prefix_controls {
    use super::*;
    /// Checks every grown completed level, so small graphs exercise prefixes.
    const EVERY_LEVEL: Schedule = Schedule {
        min_states: 2,
        growth_factor: 1,
    };
    fn input() -> (Arc<Ir>, ModelConfig, CheckOptions) {
        let source = "---- MODULE Prefix ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nP == <>(x=3)\n====\n";
        let analysis =
            dive_sema::analyze_source("Prefix.tla", source, &dive_sema::Options::default());
        assert!(analysis.accepted());
        (
            Arc::new(dive_ir::lower(&analysis).expect("IR")),
            dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
                .into_config()
                .expect("config"),
            CheckOptions::dive(7),
        )
    }
    #[test]
    fn prefix_resource_failure_keeps_actual_incomplete_evidence_and_capture_stop() {
        let (ir, config, options) = input();
        let model = bind(&ir, &config, &options).expect("model");
        let failure = run_periodic(
            &ir,
            &config,
            &options,
            &model,
            Limits {
                atom_cells: 0,
                ..Limits::default()
            },
            AcceptancePolicy::compact_default(),
            EVERY_LEVEL,
        )
        .expect_err("prefix label bound");
        assert_eq!(failure.error().code(), "DIVE-L0034");
        assert_eq!(
            failure.stage(),
            Stage::Analysis(dive_liveness::analyze::Stage::Atoms)
        );
        assert_eq!(failure.capture_completed(), Some(false));
        assert_eq!(failure.workers(1).expect("actual workers").used, 1);
        assert_eq!(
            failure
                .exploration()
                .expect("actual prefix")
                .stats
                .left_on_queue,
            1
        );
        assert_eq!(failure.periodic().expect("schedule").prefixes, 1);
        assert!(!failure.periodic().expect("schedule").final_check);
        let model = bind(&ir, &config, &options).expect("fresh model");
        let (outcome, checks) = run_periodic(
            &ir,
            &config,
            &options,
            &model,
            Limits {
                capture: capture::Limits {
                    states: 1,
                    ..capture::Limits::default()
                },
                ..Limits::default()
            },
            AcceptancePolicy::compact_default(),
            EVERY_LEVEL,
        )
        .expect("safety stop");
        assert!(
            matches!(outcome, Outcome::Safety(stop) if matches!(stop.verdict, dive_check::Verdict::ResourceLimit { .. }))
        );
        assert_eq!(checks.prefixes, 0);
        assert!(!checks.exhausted && !checks.final_check);
    }
    #[test]
    fn prefix_publication_requires_fresh_typed_and_serialized_validation() {
        let (ir, config, options) = input();
        for mutation in 0..4 {
            let model = bind(&ir, &config, &options).expect("model");
            let (analysis, checks) = dive_liveness::analyze::periodic::analyze_periodic(
                &model,
                &options,
                Limits::default(),
                AcceptancePolicy::compact_default(),
                EVERY_LEVEL,
            )
            .expect("candidate");
            let PeriodicAnalysis::Prefix(mut candidate) = analysis else {
                panic!("real prefix expected")
            };
            match mutation {
                0 => {}
                1 => candidate.states[0] = vec![Some(dive_value::Value::Int(99))].into(),
                2 => candidate.back_to = candidate.states.len(),
                _ => candidate.properties = vec!["invented-property".into()],
            }
            match validate_prefix(&ir, &config, &options, candidate) {
                Ok(Outcome::PrefixViolation(word)) => {
                    assert_eq!(mutation, 0);
                    assert_eq!(word.validation().validation, Validation::Valid);
                    assert_eq!(word.document().properties, ["P"]);
                    assert_eq!(word.candidate().exploration.stats.left_on_queue, 1);
                    let fresh = bind(&ir, &config, &options).expect("fresh model");
                    assert_eq!(
                        dive_trace::validate_doc(&fresh, word.document(), &config.model_values)
                            .0
                            .validation,
                        Validation::Valid
                    );
                }
                Err(failure) => {
                    assert_ne!(mutation, 0);
                    assert!(
                        matches!(failure.error(), Error::PrefixValidation { report, .. } if report.validation != Validation::Valid)
                    );
                    assert_eq!(failure.stage(), Stage::Validation);
                    assert_eq!(failure.capture_completed(), Some(false));
                    assert_eq!(
                        failure
                            .exploration()
                            .expect("actual prefix")
                            .stats
                            .left_on_queue,
                        1
                    );
                    assert_eq!(failure.workers(1).expect("actual width").used, 1);
                }
                other => panic!("unexpected publication outcome: {other:?}"),
            }
            assert_eq!(checks.prefixes, 1);
            assert!(!checks.exhausted);
        }
    }
}
