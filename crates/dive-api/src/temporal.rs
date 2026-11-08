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
#[cfg(test)]
use dive_liveness::analyze::periodic::Schedule;
use dive_liveness::analyze::periodic::{Checks, PrefixCandidate};
use dive_liveness::analyze::{AnalysisError, Candidate, GraphLabelPolicy, Limits};
use dive_liveness::product::AcceptancePolicy;
use dive_model::{BindOptions, Model};
use dive_trace::{
    Ending, Report as ValidationReport, Trace, TraceDoc, Validation, ValidationProgress,
    ValidationRunError,
};

mod native;
pub use native::{ValidatedNativeCounterexample, ValidatedNativePrefix};
pub(crate) use native::{core_capture_stop, core_native_report, core_source_row_failure};

/// The outcome of a bounded temporal run on Core, distinguishing real prefixes.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// All properties hold after complete native capture, without retained rows.
    NativeHolds(Box<capture::CompleteEvidence>),
    /// Independently validated native word with complete producer facts.
    NativeViolation(Box<ValidatedNativeCounterexample>),
    /// Independently validated native word with unfinished-prefix facts.
    NativePrefixViolation(Box<ValidatedNativePrefix>),
    /// The ordinary safety result when no properties are configured, when
    /// configured properties bind entirely to safety roles, or a safety stop
    /// before temporal checking completes. This preserves its verdict,
    /// statistics, output and counterexample; unsupported features and
    /// resource limits are never success.
    Safety(Box<CheckResult>),
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
    let mut limits = Limits::default();
    // File requests use machine-index capacity; explicit Rust ceilings stay exact.
    limits.product.vertices = usize::MAX;
    Configuration {
        limits,
        acceptance: AcceptancePolicy::compact_default(),
        graph_labels: GraphLabelPolicy::NoCellLimit,
    }
}

/// Ordinary core file requests use actual run budgets and checked machine layouts.
/// Planning limits and explicit library/reference limits retain their exact scope.
pub(crate) fn core_file_configuration() -> Configuration {
    let mut configuration = file_configuration();
    let limits = &mut configuration.limits;
    limits.capture = capture::Limits {
        states: usize::MAX,
        edges: usize::MAX,
        generated: u64::MAX,
        // The explorer retains fixed eight-source waves. Its per-emission
        // allocation and actual budget checks bound provisional growth.
        successors_per_unit: u64::MAX,
    };
    limits.product.vertices = usize::MAX;
    limits.product.edges = usize::MAX;
    limits.product.acceptance_cells = usize::MAX;
    limits.product.steps = usize::MAX;
    limits.atom_cells = usize::MAX;
    configuration.acceptance = AcceptancePolicy::Buffers {
        bytes: usize::MAX,
        work: usize::MAX,
    };
    configuration
}

/// A temporal result with total binding, exploration and validation time.
#[derive(Clone, Debug)]
pub struct Report {
    symmetry_liveness: Option<dive_model::SymmetryLivenessResolution>,
    outcome: Outcome,
    options: CheckOptions,
    elapsed: Duration,
    periodic: Option<Checks>,
    memory_samples: Option<dive_check::budget::MemorySamples>,
    configuration: Option<Configuration>,
}
impl Report {
    /// Original declared symmetry and effective whole-run policy after binding.
    /// `None` means no successfully bound model is available; this is not proof.
    #[must_use]
    pub fn symmetry_liveness(&self) -> Option<&dive_model::SymmetryLivenessResolution> {
        self.symmetry_liveness.as_ref()
    }

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
        Outcome::NativeHolds(capture) => {
            let mut failure = Failure::before(
                error,
                Stage::Analysis(dive_liveness::analyze::Stage::Finalization),
            );
            failure.exploration = Some(Box::new(native::facts(
                capture.safety_result(),
                capture.stats(),
            )));
            failure.elapsed = report.elapsed;
            failure.periodic = report.periodic.map(Box::new);
            failure.configuration = report.configuration.map(Box::new);
            failure.symmetry_liveness = report.symmetry_liveness.clone().map(Box::new);
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
        Outcome::NativeViolation(_) | Outcome::NativePrefixViolation(_) => None,
    }
}

/// An explicit refusal, evaluation error, resource bound or internal inconsistency. No error
/// variant is evidence of a successful model check.
#[derive(Clone, Debug)]
pub enum Error {
    /// Binding failed, preserving diagnostics and startup output.
    Binding(Box<dive_model::Failure>),
    /// A graph-independent native candidate failed independent validation.
    NativeValidation {
        /// The unvalidated selected word, without retained graph storage.
        word: Box<dive_liveness::analyze::periodic::external::NativeWord>,
        /// Actual independent validation result, which is not valid.
        report: ValidationReport,
    },
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
    /// Independent finite-error endpoint validation, distinct from a lasso.
    EndpointValidation {
        /// Actual independently computed valid/invalid/unsupported result.
        report: ValidationReport,
    },
    /// Validation exhausted an actual resource without producing a verdict.
    ValidationResource(dive_eval::EvalError),
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
            Self::NativeValidation { report, .. }
            | Self::Validation { report, .. }
            | Self::PrefixValidation { report, .. }
            | Self::EndpointValidation { report }
                if matches!(report.validation, Validation::Invalid(_)) =>
            {
                "DIVE-A0031"
            }
            Self::NativeValidation { .. }
            | Self::Validation { .. }
            | Self::PrefixValidation { .. }
            | Self::EndpointValidation { .. } => "DIVE-A0032",
            Self::ValidationResource(_) => "DIVE-L0034",
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
            Self::NativeValidation { report, .. }
            | Self::Validation { report, .. }
            | Self::PrefixValidation { report, .. }
            | Self::EndpointValidation { report } => write!(
                f,
                "{}: temporal candidate did not pass independent validation: {:?}",
                self.code(),
                report.validation
            ),
            Self::ValidationResource(error) => error.fmt(f),
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

/// A temporal failure with retained execution facts, never a verdict, as
/// [`crate::CheckReport::temporal_failure`] returns it.
#[derive(Clone, Debug)]
pub struct Failure {
    symmetry_liveness: Option<Box<dive_model::SymmetryLivenessResolution>>,
    error: Box<Error>,
    stage: Stage,
    exploration: Option<Box<dive_liveness::analyze::ExplorationEvidence>>,
    elapsed: Duration,
    action_evaluation: Option<Box<dive_liveness::analyze::ActionEvaluation>>,
    state_evaluation: Option<Box<dive_liveness::analyze::StateEvaluation>>,
    row_validation: Option<Box<RowValidation>>,
    validation_progress: Option<Box<ValidationProgress>>,
    incomplete: bool,
    periodic: Option<Box<Checks>>,
    configuration: Option<Box<Configuration>>,
    binding_output: Vec<dive_model::Message>,
}

#[derive(Clone, Debug)]
struct RowValidation {
    document: TraceDoc,
    report: ValidationReport,
}

impl Failure {
    pub(crate) fn capture_row_error(
        &self,
    ) -> Option<(&dive_eval::EvalError, &TraceDoc, &ValidationReport)> {
        let state = self.row_candidate()?;
        let validation = self.row_validation.as_deref()?;
        (validation.report.validation == Validation::Valid).then_some((
            state.core_cause()?,
            &validation.document,
            &validation.report,
        ))
    }

    fn row_candidate(&self) -> Option<&dive_liveness::analyze::StateEvaluation> {
        let state = self.state_evaluation()?;
        let cause = state.core_cause()?;
        let result = match state.site() {
            dive_liveness::analyze::StateEvaluationSite::CaptureRow
                if cause.is_eval_exception()
                    && matches!(
                        cause.ec(),
                        dive_eval::Ec::DivisionByZero | dive_eval::Ec::ModuleEvaluating
                    ) =>
            {
                state.capture_row()?.result()
            }
            dive_liveness::analyze::StateEvaluationSite::CaptureSourceRow
                if cause.exception_kind() == Some(dive_eval::ExceptionKind::ToolRuntime)
                    && cause.ec() == dive_eval::Ec::General =>
            {
                state.capture_source_row()?.result()
            }
            _ => return None,
        };
        (cause.class() == dive_eval::ErrorClass::Eval
            && cause.origin().is_some()
            && result.stats.is_some()
            && result
                .counterexample
                .as_ref()
                .is_some_and(|path| !path.states.is_empty()))
        .then_some(state)
    }
    /// Original declared symmetry and effective whole-run policy after binding.
    /// `None` means no successfully bound model is available; this is not proof.
    #[must_use]
    pub fn symmetry_liveness(&self) -> Option<&dive_model::SymmetryLivenessResolution> {
        self.symmetry_liveness.as_deref()
    }

    /// Actual configured owner limits when admitted by the enclosing API.
    /// Internal failures without a configuration retain `None`, never guessed
    /// default settings. This does not certify that any bound was exercised.
    #[must_use]
    pub fn configuration(&self) -> Option<Configuration> {
        self.configuration.as_deref().copied()
    }
    pub(crate) fn before(error: Error, stage: Stage) -> Self {
        Self {
            error: Box::new(error),
            stage,
            exploration: None,
            elapsed: Duration::ZERO,
            action_evaluation: None,
            state_evaluation: None,
            row_validation: None,
            validation_progress: None,
            incomplete: false,
            periodic: None,
            configuration: None,
            binding_output: Vec::new(),
            symmetry_liveness: None,
        }
    }
    /// The actual typed active action-label error, when one occurred.
    /// This does not claim TLC's error ordering across multiple properties.
    #[must_use]
    pub fn action_evaluation(&self) -> Option<&dive_liveness::analyze::ActionEvaluation> {
        self.action_evaluation.as_deref()
    }
    /// Original state-atom cause and postcapture/periodic/attribution site.
    /// This is not certified TLC demand or a new evaluation-error verdict.
    ///
    /// ```
    /// # fn example(failure: &dive_api::temporal::Failure) {
    /// if let Some(state) = failure.state_evaluation() {
    ///     let _actual_site = state.site();
    ///     let _original_cause = state.core_cause();
    /// }
    /// # }
    /// ```
    #[must_use]
    pub fn state_evaluation(&self) -> Option<&dive_liveness::analyze::StateEvaluation> {
        self.state_evaluation.as_deref()
    }
    /// Output retained from the binding invocation which failed.
    ///
    /// For validation rebinding this is replay evidence, not additional output
    /// from the original execution. Report writers retain the capture's output.
    #[must_use]
    pub fn binding_output(&self) -> &[dive_model::Message] {
        match self.error() {
            Error::Binding(failure) => &failure.output,
            Error::Analysis(AnalysisError::StartupTautology { output }) => output,
            _ => &self.binding_output,
        }
    }

    /// Actual progress from an interrupted independent validation invocation.
    #[must_use]
    pub fn validation_progress(&self) -> Option<&ValidationProgress> {
        self.validation_progress.as_deref()
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
            symmetry_liveness: options.symmetry_liveness,
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
        dive_model::ControlledBindError::EvaluationFailure { error, .. } => match error.class() {
            dive_eval::ErrorClass::ResourceLimit => Error::ValidationResource(error),
            _ => Error::Thread(format!("intrinsic binding evaluator failure: {error}")),
        },
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
            symmetry_liveness: options.symmetry_liveness,
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
        failure.validation_progress = Some(Box::new(ValidationProgress {
            checks_passed: 0,
            state: None,
            check: None,
        }));
    }
    failure
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
    let (error, progress) = match error {
        ValidationRunError::Interrupted { error, progress } => (error, progress),
        ValidationRunError::ResourceLimit { error, progress } => {
            return (Error::ValidationResource(error), Some(progress));
        }
        ValidationRunError::EvaluationFailure { error, progress } => {
            return (
                Error::Thread(format!("independent validation evaluator failed: {error}")),
                Some(progress),
            );
        }
        ValidationRunError::BindingFailure { error, progress } => {
            return (
                Error::Thread(format!("independent validation binding failed: {error}")),
                Some(progress),
            );
        }
        ValidationRunError::ThreadFailure(message) => return (Error::Thread(message), None),
        ValidationRunError::ControlConflict(_) => {
            return (
                Error::Analysis(AnalysisError::Inconsistent(
                    "validation control ownership conflict",
                )),
                None,
            );
        }
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

fn row_ending(
    model: &Model,
    state: &dive_liveness::analyze::StateEvaluation,
    cause: &dive_eval::EvalError,
) -> Result<Ending, (Error, Option<ValidationProgress>)> {
    Ok(
        if state.site() == dive_liveness::analyze::StateEvaluationSite::CaptureSourceRow {
            let endpoint = dive_trace::TemporalSourceEndpoint::from_error(model, cause)
                .ok_or_else(|| {
                    (
                        Error::Document("source-row endpoint lost its source context".into()),
                        None,
                    )
                })?;
            Ending::TemporalSourceError {
                ec: cause.ec().name().into(),
                endpoint,
            }
        } else {
            let endpoint =
                dive_trace::TemporalStateEndpoint::from_error(model, cause).ok_or_else(|| {
                    (
                        Error::Document("row endpoint has no original source context".into()),
                        None,
                    )
                })?;
            Ending::TemporalStateError {
                ec: cause.ec().name().into(),
                endpoint,
            }
        },
    )
}

fn validate_row(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    state: &dive_liveness::analyze::StateEvaluation,
) -> Result<RowValidation, (Error, Option<ValidationProgress>)> {
    let result = state
        .capture_row()
        .map(dive_liveness::analyze::CaptureRowEvidence::result)
        .or_else(|| {
            state
                .capture_source_row()
                .map(dive_liveness::analyze::SourceRowEvidence::result)
        })
        .ok_or_else(|| {
            (
                Error::Analysis(AnalysisError::Inconsistent("missing row certificate")),
                None,
            )
        })?;
    let path = result.counterexample.as_ref().ok_or_else(|| {
        (
            Error::Analysis(AnalysisError::Inconsistent("missing row source path")),
            None,
        )
    })?;
    let control = validation_control(options).map_err(|error| (error, None))?;
    let recipe = dive_model::Recipe::new(
        ir.clone(),
        config.clone(),
        options.eval,
        options.no_deadlock,
    )
    .with_symmetry_liveness(options.symmetry_liveness);
    let mode = if options.profile == dive_eval::Profile::Tlc {
        dive_stdlib::random::RandomMode::Tlc
    } else {
        dive_stdlib::random::RandomMode::Dive
    };
    let host = Arc::new(dive_stdlib::host::RunHost::new(
        dive_stdlib::random::Streams::new(options.seed, mode),
        None,
    ));
    // Encoding still uses a genuine fresh model, but no legacy binding or
    // Behavior validation may erase an intrinsic error before the fallible path.
    let model = recipe
        .bind_fallible(host, Some(control.clone()))
        .map_err(|failure| (*controlled_binding_failure(failure, &control).error, None))?;
    let mut states = Vec::new();
    for state in &path.states {
        if state.text.is_some() {
            return Err((
                Error::Document("incomplete row-error source state".into()),
                None,
            ));
        }
        states.push(Arc::from(
            state.values.iter().cloned().map(Some).collect::<Vec<_>>(),
        ));
    }
    let trace = Trace {
        length: Some(states.len()),
        labels: vec![None; states.len()],
        states,
        ending: Ending::Behavior,
        properties: Vec::new(),
    };
    let cause = state.core_cause().ok_or_else(|| {
        (
            Error::Document("row endpoint lost its original core cause".into()),
            None,
        )
    })?;
    let ending = row_ending(&model, state, cause)?;
    let document = trace
        .to_doc(&model)
        .map_err(|error| (Error::Document(error), None))?
        .with_ending(ending);
    let session = dive_trace::Session::new(recipe, None);
    let (report, _) = session
        .read_controlled(&document, control.clone())
        .map_err(|error| validation_run_error(&control, error))?;
    if report.validation != Validation::Valid {
        return Err((Error::EndpointValidation { report }, None));
    }
    Ok(RowValidation { document, report })
}

fn finish_row_failure(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    mut failure: Failure,
) -> Failure {
    let Some(state) = failure.row_candidate() else {
        return failure;
    };
    let validation = validate_row(ir, config, options, state);
    match validation {
        Ok(validation) => failure.row_validation = Some(Box::new(validation)),
        Err((error, progress)) => {
            failure.error = Box::new(error);
            failure.stage = Stage::Validation;
            failure.validation_progress = progress.map(Box::new);
        }
    }
    failure
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

/// The safety stop or typed failure of a periodic analysis that failed.
fn finish_periodic_failure(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    failure: dive_liveness::analyze::periodic::PeriodicFailure,
) -> Result<(Outcome, Checks), Failure> {
    let mut detailed = *failure.failure;
    let state_evaluation = detailed.take_state_evaluation();
    let (detail, action_evaluation) = detailed.into_parts();
    if let AnalysisError::Exploration(result) = detail.error {
        return Ok((Outcome::Safety(result), failure.checks));
    }
    Err(finish_row_failure(
        ir,
        config,
        options,
        Failure {
            error: Box::new(Error::Analysis(detail.error)),
            stage: Stage::Analysis(detail.stage),
            exploration: detail.exploration.or(failure.prefix),
            elapsed: Duration::ZERO,
            action_evaluation,
            state_evaluation,
            row_validation: None,
            validation_progress: None,
            incomplete: failure.capture_completed == Some(false),
            periodic: Some(Box::new(failure.checks)),
            configuration: None,
            binding_output: Vec::new(),
            symmetry_liveness: None,
        },
    ))
}

/// The report of a temporal check whose graph the core engine produced
/// (`docs/developer/core/11-liveness.md` §6): the analysis result goes
/// through the reference engine's own validation and report path, with
/// the file configuration. `periodic` says whether the default profile's
/// completed-level schedule ran.
pub(crate) fn core_report(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    failure: dive_liveness::analyze::periodic::PeriodicFailure,
    periodic: bool,
    symmetry_liveness: dive_model::SymmetryLivenessResolution,
    started: Instant,
) -> Result<Report, Failure> {
    match finish_periodic_failure(ir, config, options, failure) {
        Ok((outcome, checks)) => Ok(Report {
            outcome,
            symmetry_liveness: Some(symmetry_liveness),
            options: options.clone(),
            elapsed: started.elapsed(),
            periodic: periodic.then_some(checks),
            configuration: Some(file_configuration()),
            memory_samples: options.budget.memory_samples(),
        }),
        Err(mut failure) => {
            failure.elapsed = started.elapsed();
            failure.configuration = Some(Box::new(file_configuration()));
            failure.symmetry_liveness = Some(Box::new(symmetry_liveness));
            if !periodic {
                failure.periodic = None;
            }
            Err(failure)
        }
    }
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    clippy::panic,
    reason = "test setup and assertions"
)]
mod evidence_controls {
    use super::*;

    /// Core's temporal report for `P == property`, under `--compat=tlc`.
    fn sampled_report(property: &str) -> Report {
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
        let directory = std::env::temp_dir().join(format!(
            "dive-temporal-boundaries-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&directory).expect("directory");
        let file = directory.join("Boundaries.tla");
        std::fs::write(
            &file,
            format!(
                "---- MODULE Boundaries ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == {property}\n====\n"
            ),
        )
        .expect("module");
        std::fs::write(
            file.with_extension("cfg"),
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        )
        .expect("configuration");
        let report = crate::check_file(&crate::CheckRequest {
            file,
            compat_tlc: true,
            seed: Some(0),
            workers: Some(1),
            ..crate::CheckRequest::default()
        })
        .expect("report");
        let _ = std::fs::remove_dir_all(&directory);
        report
            .temporal_result()
            .expect("real temporal result")
            .clone()
    }

    #[test]
    fn a_terminal_measurement_failure_never_publishes_holds() {
        let mut report = sampled_report("[]<>(x \\/ ~x)");
        assert!(matches!(report.outcome(), Outcome::NativeHolds(_)));
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
        let Outcome::NativeViolation(before) = report.outcome() else {
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
        let Outcome::NativeViolation(after) = report.outcome() else {
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
#[path = "binding_controls.rs"]
pub(crate) mod binding_controls;
