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
use dive_liveness::analyze::{Analysis, AnalysisError, Candidate, GraphLabelPolicy, Limits};
use dive_liveness::product::AcceptancePolicy;
use dive_model::{BindOptions, Model};
use dive_trace::{
    Ending, Report as ValidationReport, Trace, TraceDoc, Validation, ValidationProgress,
    ValidationRunError,
};

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

/// Configured analysis owners, independent of whether execution reached them.
/// Existing APIs retain their exact caller-supplied logical cell ceilings.
#[derive(Clone, Copy, Debug)]
pub struct Configuration {
    /// Existing formula, capture, product and attribution/word bounds.
    pub limits: Limits,
    /// Acceptance-buffer admission, independent of graph-label admission.
    pub acceptance: AcceptancePolicy,
    /// Admission for graph labels only; attribution and word bounds stay finite.
    pub graph_labels: GraphLabelPolicy,
}

pub(crate) fn file_configuration() -> Configuration {
    Configuration {
        limits: Limits::default(),
        acceptance: AcceptancePolicy::compact_default(),
        graph_labels: GraphLabelPolicy::NoCellLimit,
    }
}

/// A temporal result with total binding, exploration and validation time.
#[derive(Clone, Debug)]
pub struct Report {
    outcome: Outcome,
    options: CheckOptions,
    elapsed: Duration,
    periodic: Option<Checks>,
    memory_samples: Option<dive_check::budget::MemorySamples>,
    configuration: Option<Configuration>,
}
impl Report {
    /// Actual configured owner limits and policies. Their presence establishes
    /// neither execution nor completion; safety dispatch can leave them unused.
    #[must_use]
    pub const fn configuration(&self) -> Option<Configuration> {
        self.configuration
    }
    /// Actual observations when this temporal call returned. Nested already-armed
    /// calls return a snapshot; the enclosing file report includes its owner's
    /// later terminal observation.
    #[must_use]
    pub const fn memory_samples(&self) -> Option<dive_check::budget::MemorySamples> {
        self.memory_samples
    }

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

/// Applies the outer owner's terminal observation after independent replay.
pub(crate) fn finish_report(
    report: &mut Report,
    finished: Result<Option<dive_check::budget::Exhausted>, dive_check::budget::MeasurementError>,
    elapsed: Duration,
) -> Option<Failure> {
    report.elapsed = elapsed;
    let error = match finished {
        Ok(Some(exhausted)) => Error::Analysis(AnalysisError::Stopped(Box::new(
            exhausted.during(dive_check::budget::Phase::Finalization),
        ))),
        Err(error) => Error::Analysis(AnalysisError::BudgetMeasurement(error)),
        Ok(None) => return None,
    };
    match &mut report.outcome {
        Outcome::Holds(capture) => {
            let mut failure = Failure::after(
                error,
                Stage::Analysis(dive_liveness::analyze::Stage::Finalization),
                capture,
            );
            failure.elapsed = report.elapsed;
            failure.periodic = report.periodic.map(Box::new);
            failure.configuration = report.configuration.map(Box::new);
            Some(failure)
        }
        Outcome::Safety(result) => {
            dive_check::budget::finish_result(
                result,
                finished,
                dive_check::budget::Phase::Finalization,
            );
            None
        }
        // These witnesses already passed fresh independent validation.
        Outcome::Violation(_) | Outcome::PrefixViolation(_) => None,
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
    /// Budget admission before model binding or capture starts.
    Startup,
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
            Self::Startup => "check-startup",
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
    validation_progress: Option<ValidationProgress>,
    incomplete: bool,
    periodic: Option<Box<Checks>>,
    configuration: Option<Box<Configuration>>,
    binding_output: Vec<dive_model::Message>,
}
impl Failure {
    /// Actual configured owner limits when admitted by the enclosing API.
    /// Internal failures without a configuration retain `None`, never guessed
    /// default settings. This does not certify that any bound was exercised.
    #[must_use]
    pub fn configuration(&self) -> Option<Configuration> {
        self.configuration.as_deref().copied()
    }
    fn before(error: Error, stage: Stage) -> Self {
        Self {
            error: Box::new(error),
            stage,
            exploration: None,
            elapsed: Duration::ZERO,
            action_evaluation: None,
            validation_progress: None,
            incomplete: false,
            periodic: None,
            configuration: None,
            binding_output: Vec::new(),
        }
    }
    fn after(error: Error, stage: Stage, capture: &capture::Complete) -> Self {
        Self {
            error: Box::new(error),
            stage,
            exploration: Some(Box::new(capture.into())),
            elapsed: Duration::ZERO,
            action_evaluation: None,
            validation_progress: None,
            incomplete: false,
            periodic: None,
            configuration: None,
            binding_output: Vec::new(),
        }
    }
    /// The actual typed active action-label error, when one occurred.
    /// This does not claim TLC's error ordering across multiple properties.
    #[must_use]
    pub fn action_evaluation(&self) -> Option<&dive_liveness::analyze::ActionEvaluation> {
        self.action_evaluation.as_deref()
    }
    /// Output retained from the binding invocation which failed.
    ///
    /// For validation rebinding this is replay evidence, not additional output
    /// from the original execution. Report writers retain the capture's output.
    #[must_use]
    pub fn binding_output(&self) -> &[dive_model::Message] {
        if let Error::Binding(failure) = self.error() {
            &failure.output
        } else {
            &self.binding_output
        }
    }

    /// Actual progress from an interrupted independent validation invocation.
    #[must_use]
    pub fn validation_progress(&self) -> Option<&ValidationProgress> {
        self.validation_progress.as_ref()
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
            Stage::Startup
                | Stage::Binding
                | Stage::Analysis(dive_liveness::analyze::Stage::Planning)
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

#[cfg(test)]
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

fn controlled_binding_failure(
    failure: dive_model::ControlledBindError,
    control: &dive_check::budget::EvaluationControl,
) -> Failure {
    let output = failure.output().to_vec();
    let error = match failure {
        dive_model::ControlledBindError::Binding(failure) => Error::Binding(failure),
        dive_model::ControlledBindError::Interrupted { error, .. } => {
            Error::Analysis(match control.verdict(&error) {
                Some(dive_check::Verdict::Stopped(stopped)) => AnalysisError::Stopped(stopped),
                Some(dive_check::Verdict::BudgetMeasurement { error }) => {
                    AnalysisError::BudgetMeasurement(error)
                }
                _ => AnalysisError::Inconsistent(
                    "binding interruption lacks matching active resource authority",
                ),
            })
        }
        dive_model::ControlledBindError::ControlConflict { .. } => Error::Analysis(
            AnalysisError::Inconsistent("binding control ownership conflict"),
        ),
    };
    let mut failure = Failure::before(error, Stage::Binding);
    failure.binding_output = output;
    failure
}

fn bind_controlled(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    control: &Arc<dive_check::budget::EvaluationControl>,
) -> Result<Model, Failure> {
    if options.seen_disk.is_some() {
        return Err(Failure::before(
            Error::Analysis(AnalysisError::Unsupported(
                "disk seen-state retention with temporal capture is not implemented",
            )),
            Stage::Binding,
        ));
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
    dive_model::bind_controlled(
        ir.clone(),
        config,
        &BindOptions {
            eval: options.eval,
            no_deadlock: options.no_deadlock,
            host: Some(host),
        },
        control.clone(),
    )
    .map_err(|failure| controlled_binding_failure(failure, control))
}

fn validation_binding_failure(mut failure: Failure) -> Failure {
    failure.stage = Stage::Validation;
    if matches!(
        failure.error(),
        Error::Analysis(AnalysisError::Stopped(_) | AnalysisError::BudgetMeasurement(_))
    ) {
        failure.validation_progress = Some(ValidationProgress {
            checks_passed: 0,
            state: None,
            check: None,
        });
    }
    failure
}

fn final_binding_validation_failure(failure: Failure, capture: &capture::Complete) -> Failure {
    let mut failure = validation_binding_failure(failure);
    failure.exploration = Some(Box::new(capture.into()));
    failure
}

fn prefix_binding_validation_failure(failure: Failure, capture: &capture::Prefix) -> Failure {
    let mut failure = validation_binding_failure(failure);
    failure.exploration = Some(Box::new(capture.into()));
    failure.incomplete = true;
    failure
}

fn run(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    configuration: Configuration,
) -> Result<Outcome, Failure> {
    let control = dive_check::budget::EvaluationControl::new(
        &options.budget,
        dive_check::budget::Phase::Binding,
    )
    .map_err(|_| {
        Failure::before(
            Error::Analysis(AnalysisError::Inconsistent("binding control identity")),
            Stage::Binding,
        )
    })?;
    let model =
        match dive_check::final_roles::dispatch_controlled(ir, config, options, control.clone()) {
            Some(Ok(dive_check::final_roles::Dispatch::Safety(result))) => {
                return Ok(Outcome::Safety(result));
            }
            Some(Ok(dive_check::final_roles::Dispatch::Temporal(model))) => *model,
            Some(Err(error)) => return Err(controlled_binding_failure(error, &control)),
            None => bind_controlled(ir, config, options, &control)?,
        };
    run_bound(ir, config, options, &model, configuration)
}

fn run_bound(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    model: &Model,
    configuration: Configuration,
) -> Result<Outcome, Failure> {
    finish_final(
        ir,
        config,
        options,
        dive_liveness::analyze::analyze_final_with_policies(
            model,
            options,
            configuration.limits,
            configuration.acceptance,
            configuration.graph_labels,
        ),
    )
}

fn validation_control(
    options: &CheckOptions,
) -> Result<Arc<dive_check::budget::EvaluationControl>, Error> {
    dive_check::budget::EvaluationControl::new(
        &options.budget,
        dive_check::budget::Phase::Validation,
    )
    .map_err(|_| Error::Analysis(AnalysisError::Inconsistent("validation control identity")))
}

fn validation_checkpoint(
    control: &dive_check::budget::EvaluationControl,
) -> Result<(), ValidationRunError> {
    dive_eval::HostControl::check(control).map_err(|interrupted| ValidationRunError::Interrupted {
        error: dive_eval::EvalError::interrupted(interrupted),
        progress: ValidationProgress {
            checks_passed: 0,
            state: None,
            check: None,
        },
    })
}

fn validation_run_error(
    control: &dive_check::budget::EvaluationControl,
    error: ValidationRunError,
) -> (Error, Option<ValidationProgress>) {
    let ValidationRunError::Interrupted { error, progress } = error else {
        return (
            Error::Analysis(AnalysisError::Inconsistent(
                "validation control ownership conflict",
            )),
            None,
        );
    };
    let error = match control.verdict(&error) {
        Some(dive_check::Verdict::Stopped(stopped)) => AnalysisError::Stopped(stopped),
        Some(dive_check::Verdict::BudgetMeasurement { error }) => {
            AnalysisError::BudgetMeasurement(error)
        }
        _ => {
            AnalysisError::Inconsistent("validation interruption lacks matching resource authority")
        }
    };
    (Error::Analysis(error), Some(progress))
}

fn final_validation_run_failure(
    control: &dive_check::budget::EvaluationControl,
    error: ValidationRunError,
    capture: &capture::Complete,
) -> Failure {
    let (error, progress) = validation_run_error(control, error);
    let mut failure = Failure::after(error, Stage::Validation, capture);
    failure.validation_progress = progress;
    failure
}

// The API creates native documents itself. Keep this deliberately separate
// from TLC's ambiguous document/session reader, which is not controlled yet.
fn validate_native_document(
    model: &Model,
    document: &TraceDoc,
    control: &Arc<dive_check::budget::EvaluationControl>,
) -> Result<ValidationReport, ValidationRunError> {
    validation_checkpoint(control)?;
    let trace = match Trace::decode(model, document) {
        Ok(trace) => trace,
        Err(error) => {
            return Ok(ValidationReport {
                validation: if model.has_runtime_names() {
                    Validation::Unsupported(dive_trace::RUNTIME_NAMES_UNSUPPORTED.into())
                } else {
                    Validation::Invalid(dive_trace::Failure {
                        state: error.state.unwrap_or(0),
                        check: dive_trace::Check::Shape,
                        detail: error.detail,
                    })
                },
                notes: Vec::new(),
                length: document.states.len(),
            });
        }
    };
    dive_trace::validate_controlled(model, &trace, control.clone())
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "actual candidate control evidence"
)]
mod validation_controls {
    use super::*;

    fn input() -> (Arc<Ir>, ModelConfig) {
        let source = "---- MODULE ValidationBudget ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nP == <>(x=3)\n====\n";
        let analysis = dive_sema::analyze_source(
            "ValidationBudget.tla",
            source,
            &dive_sema::Options::default(),
        );
        assert!(analysis.accepted());
        (
            Arc::new(dive_ir::lower(&analysis).expect("IR")),
            dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
                .into_config()
                .expect("configuration"),
        )
    }

    fn assert_validation_stop(failure: &Failure, complete: bool, actual: dive_check::Stats) {
        assert_eq!(failure.stage(), Stage::Validation);
        assert_eq!(failure.capture_completed(), Some(complete));
        assert_eq!(failure.exploration().expect("actual capture").stats, actual);
        let Error::Analysis(AnalysisError::Stopped(stopped)) = failure.error() else {
            panic!("typed stop")
        };
        assert_eq!(stopped.phase, dive_check::budget::Phase::Validation);
        assert_eq!(stopped.resource, dive_check::budget::Resource::Time);
        let progress = failure
            .validation_progress()
            .expect("actual validation admission");
        assert_eq!(progress.checks_passed, 0);
        assert_eq!(progress.state, None);
        assert_eq!(progress.check, None);
    }

    #[test]
    fn an_uncertified_final_candidate_stops_with_real_capture_and_no_violation() {
        let (ir, config) = input();
        for mut options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            let model = bind(&ir, &config, &options).expect("model");
            let Analysis::Candidate(candidate) = dive_liveness::analyze::analyze_final_with_policy(
                &model,
                &options,
                Limits::default(),
                AcceptancePolicy::compact_default(),
            )
            .expect("real analysis") else {
                panic!("real candidate")
            };
            let actual = candidate.exploration.stats;
            let watchdog = dive_check::budget::Budget::new(Some(Duration::ZERO), None)
                .start()
                .expect("thread")
                .expect("watchdog");
            options.budget = watchdog.budget().clone();
            let failure = finish_final(&ir, &config, &options, Ok(Analysis::Candidate(candidate)))
                .expect_err("uncertified candidate cannot become violation");
            assert_validation_stop(&failure, true, actual);
        }
    }

    #[test]
    fn an_uncertified_prefix_candidate_keeps_actual_incomplete_evidence() {
        let (ir, config) = input();
        for mut options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
            let model = bind(&ir, &config, &options).expect("model");
            let (PeriodicAnalysis::Prefix(candidate), _) =
                dive_liveness::analyze::periodic::analyze_periodic(
                    &model,
                    &options,
                    Limits::default(),
                    AcceptancePolicy::compact_default(),
                    Schedule {
                        min_states: 2,
                        growth_factor: 1,
                    },
                )
                .expect("real periodic analysis")
            else {
                panic!("real prefix candidate")
            };
            let actual = candidate.exploration.stats;
            let watchdog = dive_check::budget::Budget::new(Some(Duration::ZERO), None)
                .start()
                .expect("thread")
                .expect("watchdog");
            options.budget = watchdog.budget().clone();
            let failure = validate_prefix(&ir, &config, &options, candidate)
                .expect_err("uncertified prefix cannot become violation");
            assert_validation_stop(&failure, false, actual);
        }
    }

    #[test]
    fn decoded_native_shape_errors_keep_the_independent_validation_class() {
        let (ir, config) = input();
        let options = CheckOptions::tlc(0, 0);
        let model = bind(&ir, &config, &options).expect("model");
        let document = TraceDoc::behavior(vec![serde_json::json!({"wrong":0})]).expect("document");
        let legacy = dive_trace::validate_doc(&model, &document, &config.model_values).0;
        let control = validation_control(&options).expect("control");
        let controlled =
            validate_native_document(&model, &document, &control).expect("shape report");
        assert_eq!(controlled, legacy);
        assert!(
            matches!(controlled.validation,Validation::Invalid(ref e) if e.check==dive_trace::Check::Shape)
        );
    }

    #[test]
    fn an_unarmed_validation_identity_is_not_resource_authority() {
        let mut options = CheckOptions::tlc(0, 0);
        options.budget = dive_check::budget::Budget::new(Some(Duration::from_secs(3600)), None);
        let control = validation_control(&options).expect("unarmed control");
        assert!(dive_eval::HostControl::check(control.as_ref()).is_ok());
        // Unarmed contexts carry the public no-owner identity zero. A matching
        // payload still cannot prove that any run resource was exhausted.
        let (error, _) = validation_run_error(
            &control,
            ValidationRunError::Interrupted {
                error: dive_eval::EvalError::interrupted(dive_eval::Interrupted::new(0)),
                progress: ValidationProgress {
                    checks_passed: 0,
                    state: None,
                    check: None,
                },
            },
        );
        assert!(matches!(
            error,
            Error::Analysis(AnalysisError::Inconsistent(_))
        ));
    }

    #[test]
    fn foreign_interruption_cannot_borrow_an_exhausted_validation_budget() {
        let watchdog = dive_check::budget::Budget::new(Some(Duration::ZERO), None)
            .start()
            .expect("thread")
            .expect("watchdog");
        let mut options = CheckOptions::tlc(0, 0);
        options.budget = watchdog.budget().clone();
        let control = validation_control(&options).expect("control");
        let (error, _) = validation_run_error(
            &control,
            ValidationRunError::Interrupted {
                error: dive_eval::EvalError::interrupted(dive_eval::Interrupted::new(0)),
                progress: ValidationProgress {
                    checks_passed: 0,
                    state: None,
                    check: None,
                },
            },
        );
        assert!(matches!(
            error,
            Error::Analysis(AnalysisError::Inconsistent(_))
        ));
    }
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
            let control = validation_control(options).map_err(|error| {
                Failure::after(error, Stage::Validation, &candidate.exploration)
            })?;
            validation_checkpoint(&control).map_err(|error| {
                final_validation_run_failure(&control, error, &candidate.exploration)
            })?;
            // Rebind separately: validation cannot inspect the checker's graph
            // or depend on its mutable evaluator/host state.
            let validation_model =
                bind_controlled(ir, config, options, &control).map_err(|failure| {
                    final_binding_validation_failure(failure, &candidate.exploration)
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
            let report =
                dive_trace::validate_controlled(&validation_model, &trace, control.clone())
                    .map_err(|error| {
                        final_validation_run_failure(&control, error, &candidate.exploration)
                    })?;
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
            let report = validate_native_document(&validation_model, &document, &control).map_err(
                |error| final_validation_run_failure(&control, error, &candidate.exploration),
            )?;
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
                    validation_progress: None,
                    incomplete: false,
                    periodic: None,
                    configuration: None,
                    binding_output: Vec::new(),
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
        validation_progress: None,
        incomplete: false,
        periodic: None,
        configuration: None,
        binding_output: Vec::new(),
    }
}

fn prefix_failure(error: Error, stage: Stage, prefix: &capture::Prefix) -> Failure {
    Failure {
        error: Box::new(error),
        stage,
        exploration: Some(Box::new(prefix.into())),
        elapsed: Duration::ZERO,
        action_evaluation: None,
        validation_progress: None,
        incomplete: true,
        periodic: None,
        configuration: None,
        binding_output: Vec::new(),
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
        validation_progress: None,
        incomplete: true,
        periodic: None,
        configuration: None,
        binding_output: Vec::new(),
    }
}

fn prefix_validation_run_failure(
    control: &dive_check::budget::EvaluationControl,
    error: ValidationRunError,
    prefix: &capture::Prefix,
) -> Failure {
    let (error, progress) = validation_run_error(control, error);
    let mut failure = prefix_failure(error, Stage::Validation, prefix);
    failure.validation_progress = progress;
    failure
}

fn validate_prefix(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    candidate: PrefixCandidate,
) -> Result<Outcome, Failure> {
    // Same independent checks as final candidates, with distinct prefix evidence.
    let control = validation_control(options)
        .map_err(|error| prefix_failure(error, Stage::Validation, &candidate.exploration))?;
    validation_checkpoint(&control)
        .map_err(|error| prefix_validation_run_failure(&control, error, &candidate.exploration))?;
    let model = bind_controlled(ir, config, options, &control)
        .map_err(|failure| prefix_binding_validation_failure(failure, &candidate.exploration))?;
    let trace = Trace {
        states: candidate.states.clone(),
        labels: vec![None; candidate.states.len()],
        ending: Ending::Lasso {
            back_to: candidate.back_to,
        },
        length: Some(candidate.states.len()),
        properties: candidate.properties.clone(),
    };
    let report = dive_trace::validate_controlled(&model, &trace, control.clone())
        .map_err(|error| prefix_validation_run_failure(&control, error, &candidate.exploration))?;
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
    let report = validate_native_document(&model, &document, &control)
        .map_err(|error| prefix_validation_run_failure(&control, error, &candidate.exploration))?;
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
    schedule: Schedule,
    configuration: Configuration,
) -> Result<(Outcome, Checks), Failure> {
    let (analysis, checks) = match dive_liveness::analyze::periodic::analyze_periodic_with_policies(
        model,
        options,
        configuration.limits,
        configuration.acceptance,
        configuration.graph_labels,
        schedule,
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
                validation_progress: None,
                incomplete: failure.capture_completed == Some(false),
                periodic: Some(Box::new(failure.checks)),
                configuration: None,
                binding_output: Vec::new(),
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
                let Ok(control) = dive_check::budget::EvaluationControl::new(
                    &options.budget,
                    dive_check::budget::Phase::Binding,
                ) else {
                    return Some(Err(Failure::before(
                        Error::Analysis(AnalysisError::Inconsistent("binding control identity")),
                        Stage::Binding,
                    )));
                };
                dive_check::final_roles::dispatch_ordinary_controlled(
                    ir,
                    config,
                    options,
                    control.clone(),
                )
                .map(|result| {
                    let bound =
                        result.map_err(|failure| controlled_binding_failure(failure, &control))?;
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
                                    schedule,
                                    file_configuration(),
                                )
                                .map(|(outcome, checks)| (outcome, Some(checks)))
                            } else {
                                run_bound(ir, config, options, &model, file_configuration())
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
                    configuration: Some(file_configuration()),
                    memory_samples: options.budget.memory_samples(),
                })
                .map_err(|mut failure| {
                    failure.elapsed = started.elapsed();
                    failure.configuration = Some(Box::new(file_configuration()));
                    failure
                }),
        ),
        Err(mut failure) => {
            failure.elapsed = started.elapsed();
            failure.configuration = Some(Box::new(file_configuration()));
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
    check_final_with_policies(
        ir,
        config,
        options,
        limits,
        policy,
        GraphLabelPolicy::Cells(limits.atom_cells),
    )
}

/// Like [`check_final_with_policy`], explicitly selecting graph-label admission.
/// Every existing entrypoint retains its supplied graph cell ceiling, including
/// zero and an explicitly supplied default value. No other owner is changed.
///
/// ```no_run
/// # fn example(ir: &std::sync::Arc<dive_ir::Ir>, cfg: &dive_config::ModelConfig) {
/// use dive_liveness::{analyze::{GraphLabelPolicy, Limits}, product::AcceptancePolicy};
/// let result = dive_api::temporal::check_final_with_policies(ir, cfg,
///     &dive_check::CheckOptions::tlc(0, 0), Limits::default(),
///     AcceptancePolicy::compact_default(), GraphLabelPolicy::NoCellLimit);
/// // Violations, if any, have passed both independent native validations.
/// # let _ = result;
/// # }
/// ```
///
/// # Errors
/// The same typed failures and retained execution facts as [`check_final_with_policy`].
pub fn check_final_with_policies(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    limits: Limits,
    policy: AcceptancePolicy,
    graph_labels: GraphLabelPolicy,
) -> Result<Report, Failure> {
    let configuration = Configuration {
        limits,
        acceptance: policy,
        graph_labels,
    };
    check_final_configured(ir, config, options, configuration)
        .map(|mut report| {
            report.configuration = Some(configuration);
            report
        })
        .map_err(|mut failure| {
            failure.configuration = Some(Box::new(configuration));
            failure
        })
}

fn check_final_configured(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    configuration: Configuration,
) -> Result<Report, Failure> {
    let started = Instant::now();
    let report_options = options.clone();
    let (mut watchdog, armed) = dive_check::budget::arm(options).map_err(|error| {
        Failure::before(
            Error::Analysis(match error {
                dive_check::budget::StartError::Measurement(error) => {
                    AnalysisError::BudgetMeasurement(error)
                }
                error @ dive_check::budget::StartError::Thread(_) => {
                    AnalysisError::BudgetStartup(error.to_string())
                }
            }),
            Stage::Startup,
        )
    })?;
    let options = armed.as_ref();
    match options.budget.poll() {
        Ok(Some(exhausted)) => {
            return Err(Failure::before(
                Error::Analysis(AnalysisError::Stopped(Box::new(
                    exhausted.during(dive_check::budget::Phase::Startup),
                ))),
                Stage::Startup,
            ));
        }
        Err(error) => {
            return Err(Failure::before(
                Error::Analysis(AnalysisError::BudgetMeasurement(error)),
                Stage::Startup,
            ));
        }
        Ok(None) => {}
    }
    let outcome = if config.properties.is_empty() {
        // No temporal claim requested; retain the ordinary startup host.
        Outcome::Safety(Box::new(dive_check::check(ir, config, options)))
    } else {
        std::thread::scope(|s| {
            std::thread::Builder::new()
                .stack_size(dive_eval::RECOMMENDED_STACK)
                .spawn_scoped(s, || run(ir, config, options, configuration))
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
        })?
    };
    let mut report = Report {
        outcome,
        options: report_options,
        elapsed: started.elapsed(),
        periodic: None,
        configuration: None,
        memory_samples: None,
    };
    if let Some(watchdog) = &mut watchdog {
        let finished = watchdog.finish();
        if let Some(failure) = finish_report(&mut report, finished, started.elapsed()) {
            return Err(failure);
        }
    }
    report.memory_samples = options.budget.memory_samples();
    report.elapsed = started.elapsed();
    Ok(report)
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test setup and assertions"
)]
mod evidence_controls {
    use super::*;

    fn sampled_report(property: &str) -> Report {
        let source = format!(
            "---- MODULE Boundaries ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == {property}\n====\n"
        );
        let analysis =
            dive_sema::analyze_source("Boundaries.tla", &source, &dive_sema::Options::default());
        assert!(analysis.accepted());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("cfg");
        check_final(&ir, &cfg, &CheckOptions::tlc(0, 0), Limits::default())
            .expect("real temporal result")
    }

    #[test]
    fn a_terminal_measurement_failure_never_publishes_holds() {
        let mut report = sampled_report("[]<>(x \\/ ~x)");
        assert!(matches!(report.outcome(), Outcome::Holds(_)));
        let failure = finish_report(
            &mut report,
            Err(dive_check::budget::MeasurementError::unavailable(
                dive_check::budget::SamplePoint::Final,
            )),
            Duration::from_millis(2),
        )
        .expect("holding cannot escape");
        assert_eq!(
            failure.error().code(),
            dive_check::budget::MEASUREMENT_FAILURE_CODE
        );
        assert_eq!(failure.capture_completed(), Some(true));
        assert_eq!(failure.stage().name(), "finalization");
        assert_eq!(failure.elapsed(), Duration::from_millis(2));
        let evidence = failure.exploration().expect("actual complete capture");
        assert_eq!(
            (
                evidence.stats.distinct,
                evidence.stats.generated,
                evidence.stats.depth
            ),
            (2, 3, 2)
        );
    }

    #[test]
    fn terminal_memory_excess_has_completed_capture_at_finalization() {
        let mut report = sampled_report(r"[]<>(x \/ ~x)");
        let stop = dive_check::budget::Exhausted {
            resource: dive_check::budget::Resource::Memory,
            limit: 100,
            observed: 101,
            elapsed_ms: 2,
        };
        let failure = finish_report(&mut report, Ok(Some(stop)), Duration::from_millis(2))
            .expect("no holding verdict");
        assert_eq!(failure.stage().name(), "finalization");
        assert_eq!(failure.capture_completed(), Some(true));
        let Error::Analysis(AnalysisError::Stopped(stopped)) = failure.error() else {
            panic!("actual resource stop");
        };
        assert_eq!(stopped.phase, dive_check::budget::Phase::Finalization);
        let evidence = failure.exploration().expect("actual completed capture");
        assert_eq!(
            (
                evidence.stats.distinct,
                evidence.stats.generated,
                evidence.stats.depth
            ),
            (2, 3, 2)
        );
    }

    #[test]
    fn a_terminal_failure_keeps_an_already_validated_witness() {
        let mut report = sampled_report("<>[]x");
        let Outcome::Violation(before) = report.outcome() else {
            panic!("real validated witness");
        };
        let document = before.document().to_json();
        assert!(
            finish_report(
                &mut report,
                Err(dive_check::budget::MeasurementError::unavailable(
                    dive_check::budget::SamplePoint::Final
                )),
                Duration::from_millis(2),
            )
            .is_none()
        );
        let Outcome::Violation(after) = report.outcome() else {
            panic!("witness retained");
        };
        assert_eq!(after.document().to_json(), document);
        assert_eq!(after.validation().validation, Validation::Valid);
    }

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
            EVERY_LEVEL,
            Configuration {
                limits: Limits {
                    atom_cells: 0,
                    ..Limits::default()
                },
                acceptance: AcceptancePolicy::compact_default(),
                graph_labels: GraphLabelPolicy::Cells(0),
            },
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
            EVERY_LEVEL,
            Configuration {
                limits: Limits {
                    capture: capture::Limits {
                        states: 1,
                        ..capture::Limits::default()
                    },
                    ..Limits::default()
                },
                acceptance: AcceptancePolicy::compact_default(),
                graph_labels: GraphLabelPolicy::Cells(Limits::default().atom_cells),
            },
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

#[cfg(test)]
#[path = "binding_controls.rs"]
pub(crate) mod binding_controls;
