//! Supported temporal checking with independent counterexample validation.
//!
//! Final checking requires exhaustive capture. Ordinary checking can also stop
//! at a deterministic completed-level boundary after a freshly bound validator
//! accepts both the typed and serialized word. No prefix can establish Holds.
//! TLC lasso choice and wall-clock scheduling are not reproduced.

use std::sync::Arc;
use std::time::{Duration, Instant};

use dive_check::{CheckOptions, CheckResult, capture};
use dive_config::ModelConfig;
use dive_ir::Ir;
use dive_liveness::analyze::{Analysis, AnalysisError, Candidate, Limits};
use dive_model::{BindOptions, Model};
use dive_trace::{Ending, Report as ValidationReport, Trace, TraceDoc, Validation};

/// The outcome of a final-only temporal run.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// Every planned property holds on the fully captured graph, subject to
    /// the exploration's reported fingerprint collision qualifications.
    Holds(Box<capture::Complete>),
    /// A concrete periodic word falsifies the named properties and passes
    /// the independent validator. TLC's exact lasso selection is not claimed.
    Violation(Box<ValidatedCounterexample>),
    /// Independently validated violation found at a completed-level prefix.
    /// No exhaustive exploration or Holds is implied.
    PrefixViolation(Box<ValidatedPrefixCounterexample>),
    /// The safety engine stopped before temporal checking could complete.
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

/// A validated counterexample and the actual nonexhaustive search boundary.
/// Private fields prevent callers from manufacturing a temporal verdict.
#[derive(Clone, Debug)]
pub struct ValidatedPrefixCounterexample {
    word: dive_liveness::analyze::CandidateWord,
    exploration: capture::Prefix,
    document: TraceDoc,
    validation: ValidationReport,
}
impl ValidatedPrefixCounterexample {
    /// Attributed word with a zero-based closing target.
    #[must_use]
    pub fn word(&self) -> &dive_liveness::analyze::CandidateWord {
        &self.word
    }
    /// Actual completed-level prefix; never Complete.
    #[must_use]
    pub fn exploration(&self) -> &capture::Prefix {
        &self.exploration
    }
    /// Lossless independently validated trace document.
    #[must_use]
    pub fn document(&self) -> &TraceDoc {
        &self.document
    }
    /// Independent serialized-word validation, always Valid.
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
    schedule: dive_liveness::periodic_backend::Schedule,
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
    /// Actual deterministic schedule; no wall-clock service ordering.
    #[must_use]
    pub const fn schedule(&self) -> dive_liveness::periodic_backend::Schedule {
        self.schedule
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
    /// Independent replay refused or refuted a prefix word.
    PrefixValidation {
        /// Attributed word that did not validate.
        word: Box<dive_liveness::analyze::CandidateWord>,
        /// Independent validation result; never Valid here.
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
    error: Error,
    stage: Stage,
    exploration: Option<Box<dive_liveness::analyze::ExplorationEvidence>>,
    elapsed: Duration,
}
impl Failure {
    fn before(error: Error, stage: Stage) -> Self {
        Self {
            error,
            stage,
            exploration: None,
            elapsed: Duration::ZERO,
        }
    }
    fn after(error: Error, stage: Stage, capture: &capture::Complete) -> Self {
        Self {
            error,
            stage,
            exploration: Some(Box::new(capture.into())),
            elapsed: Duration::ZERO,
        }
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
    /// Complete capture facts, when capture finished before the failure.
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
        if let Some(exploration) = &self.exploration {
            Some(exploration.boundary.is_none())
        } else if matches!(
            self.stage,
            Stage::Binding | Stage::Analysis(dive_liveness::analyze::Stage::Planning)
        ) {
            Some(false)
        } else {
            None
        }
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
        Some(&self.error)
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
) -> Result<Outcome, Failure> {
    let model =
        bind(ir, config, options).map_err(|error| Failure::before(error, Stage::Binding))?;
    match dive_liveness::analyze::analyze_final_with_evidence(&model, options, limits) {
        Ok(Analysis::Holds(complete)) => Ok(Outcome::Holds(Box::new(complete))),
        Ok(Analysis::Candidate(candidate)) => finish_candidate(ir, config, options, candidate),
        Err(dive_liveness::analyze::Failure {
            error: AnalysisError::Exploration(result),
            ..
        }) => Ok(Outcome::Safety(result)),
        Err(failure) => Err(Failure {
            error: Error::Analysis(failure.error),
            stage: Stage::Analysis(failure.stage),
            exploration: failure.exploration,
            elapsed: Duration::ZERO,
        }),
    }
}

enum WordFailure {
    Error(Error),
    Validation(ValidationReport),
}

// Both paths rebind independently and check both the typed and serialized word.
fn validate_word(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    states: &[dive_eval::next::StateVals],
    back_to: usize,
    properties: &[String],
) -> Result<(TraceDoc, ValidationReport), WordFailure> {
    let model = bind(ir, config, options).map_err(WordFailure::Error)?;
    let trace = Trace {
        states: states.to_vec(),
        labels: vec![None; states.len()],
        ending: Ending::Lasso { back_to },
        length: Some(states.len()),
        properties: properties.to_vec(),
    };
    let report = dive_trace::validate(&model, &trace);
    if report.validation != Validation::Valid {
        return Err(WordFailure::Validation(report));
    }
    let document = trace
        .to_doc(&model)
        .map_err(|error| WordFailure::Error(Error::Document(error)))?;
    let report = dive_trace::validate_doc(&model, &document, &config.model_values).0;
    if report.validation != Validation::Valid {
        return Err(WordFailure::Validation(report));
    }
    Ok((document, report))
}

fn finish_candidate(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    candidate: Candidate,
) -> Result<Outcome, Failure> {
    match validate_word(
        ir,
        config,
        options,
        &candidate.states,
        candidate.back_to,
        &candidate.properties,
    ) {
        Ok((document, validation)) => Ok(Outcome::Violation(Box::new(ValidatedCounterexample {
            candidate,
            document,
            validation,
        }))),
        Err(error) => {
            let exploration = Some(Box::new((&candidate.exploration).into()));
            let (error, stage) = match error {
                WordFailure::Validation(report) => (
                    Error::Validation {
                        candidate: Box::new(candidate),
                        report,
                    },
                    Stage::Validation,
                ),
                WordFailure::Error(error) => {
                    let stage = if matches!(error, Error::Document(_)) {
                        Stage::Document
                    } else {
                        Stage::Validation
                    };
                    (error, stage)
                }
            };
            Err(Failure {
                error,
                stage,
                exploration,
                elapsed: Duration::ZERO,
            })
        }
    }
}

struct ValidatedWord {
    word: dive_liveness::analyze::CandidateWord,
    document: TraceDoc,
    validation: ValidationReport,
}
struct LevelConsumer<'a> {
    model: &'a Model,
    ir: &'a Arc<Ir>,
    config: &'a ModelConfig,
    options: &'a CheckOptions,
    prepared: &'a dive_liveness::analyze::Prepared,
    last: u64,
    boundary: Option<u64>,
    pending: Option<Result<ValidatedWord, Failure>>,
}
impl capture::Observer for LevelConsumer<'_> {
    fn level_control(
        &mut self,
        level: u64,
        graph: &capture::BehaviorGraph,
        stats: dive_check::Stats,
    ) -> Result<capture::LevelControl, String> {
        use dive_liveness::periodic_backend::Growth;
        if stats.left_on_queue == 0
            || !(Growth {
                size: stats.distinct,
                last: self.last,
            })
            .due(Growth::DEFAULT_THRESHOLD)
        {
            return Ok(capture::LevelControl::Continue);
        }
        let mut stage = dive_liveness::analyze::Stage::Atoms;
        let result = dive_liveness::analyze::check_graph(
            self.model,
            self.prepared,
            graph,
            self.options,
            &mut stage,
        )
        .map_err(|error| Failure::before(Error::Analysis(error), Stage::Analysis(stage)))
        .and_then(|word| match word {
            None => Ok(None),
            Some(word) => match validate_word(
                self.ir,
                self.config,
                self.options,
                &word.states,
                word.back_to,
                &word.properties,
            ) {
                Ok((document, validation)) => Ok(Some(ValidatedWord {
                    word,
                    document,
                    validation,
                })),
                Err(error) => {
                    let (error, stage) = match error {
                        WordFailure::Validation(report) => (
                            Error::PrefixValidation {
                                word: Box::new(word),
                                report,
                            },
                            Stage::Validation,
                        ),
                        WordFailure::Error(error) => {
                            let stage = if matches!(error, Error::Document(_)) {
                                Stage::Document
                            } else {
                                Stage::Validation
                            };
                            (error, stage)
                        }
                    };
                    Err(Failure::before(error, stage))
                }
            },
        });
        self.last = stats.distinct;
        match result {
            Ok(None) => Ok(capture::LevelControl::Continue),
            Ok(Some(word)) => {
                self.boundary = Some(level);
                self.pending = Some(Ok(word));
                Ok(capture::LevelControl::Stop)
            }
            Err(failure) => {
                let message = failure.to_string();
                self.boundary = Some(level);
                self.pending = Some(Err(failure));
                Err(message)
            }
        }
    }
}

fn run_model(
    model: &Model,
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    schedule: dive_liveness::periodic_backend::Schedule,
    limits: Limits,
) -> Result<Outcome, Failure> {
    use dive_liveness::analyze::{self, ExplorationEvidence};
    let prepared = analyze::prepare(model, limits).map_err(|error| {
        Failure::before(
            Error::Analysis(error),
            Stage::Analysis(analyze::Stage::Planning),
        )
    })?;
    let mut consumer = LevelConsumer {
        model,
        ir,
        config,
        options,
        prepared: &prepared,
        last: 1,
        boundary: None,
        pending: None,
    };
    let captured = if schedule == dive_liveness::periodic_backend::Schedule::Final {
        capture::explore_with_evidence(model, options, limits.capture)
    } else {
        capture::explore_observed_with_evidence(model, options, limits.capture, &mut consumer)
    }
    .map_err(|error| Failure::before(Error::Thread(error), Stage::Unknown))?;
    match captured {
        capture::Exploration::Stopped(result) => {
            if let Some(Err(mut failure)) = consumer.pending {
                if let Some(stats) = result.stats {
                    failure.exploration = Some(Box::new(ExplorationEvidence {
                        stats,
                        workers: result.workers.clone(),
                        collision: result.collision.clone(),
                        output: result.output.clone(),
                        warnings: result.warnings.clone(),
                        boundary: consumer.boundary,
                    }));
                }
                Err(failure)
            } else {
                Ok(Outcome::Safety(result))
            }
        }
        capture::Exploration::Prefix(prefix) => {
            let Some(Ok(validated)) = consumer.pending else {
                return Err(Failure::before(
                    Error::Analysis(analyze::AnalysisError::Inconsistent(
                        "consumer stopped without a validated word",
                    )),
                    Stage::Unknown,
                ));
            };
            Ok(Outcome::PrefixViolation(Box::new(
                ValidatedPrefixCounterexample {
                    word: validated.word,
                    exploration: *prefix,
                    document: validated.document,
                    validation: validated.validation,
                },
            )))
        }
        capture::Exploration::Complete(complete) => {
            let mut stage = analyze::Stage::Atoms;
            let word = analyze::check_graph(
                model,
                &prepared,
                &complete.graph,
                &complete.options,
                &mut stage,
            )
            .map_err(|error| {
                Failure::after(Error::Analysis(error), Stage::Analysis(stage), &complete)
            })?;
            match word {
                None => Ok(Outcome::Holds(complete)),
                Some(word) => finish_candidate(
                    ir,
                    config,
                    options,
                    Candidate {
                        target_property: word.target_property,
                        properties: word.properties,
                        states: word.states,
                        back_to: word.back_to,
                        edges: word.edges,
                        exploration: *complete,
                    },
                ),
            }
        }
    }
}

/// Runs ordinary safety or supported temporal checking with a deterministic
/// completed-level schedule. No-claims models use the normal safety path and
/// the same binding/host; prefix absence never establishes Holds.
///
/// ```
/// use std::sync::Arc;
/// use dive_api::temporal::{check_with_schedule_with_evidence, Outcome};
/// use dive_check::CheckOptions;
/// use dive_liveness::{analyze::Limits, periodic_backend::Schedule};
/// let a = dive_sema::analyze_source("M.tla",
///     "---- MODULE M ----\nVARIABLE x\nInit == x=TRUE\nNext == x'=~x\nP == []<>(~x)\n====\n",
///     &Default::default());
/// let ir = Arc::new(dive_ir::lower(&a)?);
/// let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n").into_config().ok_or("config")?;
/// let report = check_with_schedule_with_evidence(&ir, &cfg, &CheckOptions::dive(7), Schedule::Default, Limits::default())?;
/// assert!(matches!(report.outcome(), Outcome::PrefixViolation(v) if v.exploration().stats.left_on_queue == 1));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
///
/// # Errors
/// Planning, exact labeling, product construction, independent validation or
/// explicit bounds failed. The failure retains available actual search facts.
pub fn check_with_schedule_with_evidence(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    schedule: dive_liveness::periodic_backend::Schedule,
    limits: Limits,
) -> Result<Report, Failure> {
    let started = Instant::now();
    let outcome = match dive_check::check_with_temporal(ir, config, options, |model| {
        run_model(model, ir, config, options, schedule, limits)
    }) {
        dive_check::CheckDispatch::Safety(result) => Ok(Outcome::Safety(result)),
        dive_check::CheckDispatch::Temporal(result) => result,
    }
    .map_err(|mut failure| {
        failure.elapsed = started.elapsed();
        failure
    })?;
    Ok(Report {
        outcome,
        options: options.clone(),
        elapsed: started.elapsed(),
        schedule,
    })
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
pub fn check_final(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    limits: Limits,
) -> Result<Report, Error> {
    check_final_with_evidence(ir, config, options, limits).map_err(|failure| failure.error)
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
    let started = Instant::now();
    let outcome = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(s, || run(ir, config, options, limits))
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
        schedule: dive_liveness::periodic_backend::Schedule::Final,
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
