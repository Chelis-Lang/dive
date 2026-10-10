//! Bounded final temporal checking for Rust callers.
//!
//! This API checks the supported pure temporal subset and independently validates
//! every periodic counterexample before returning it as a violation. It binds a
//! fresh model for each run and for validation. It selects a deterministic short
//! lasso, including with TLC value semantics; TLC-exact selection and periodic
//! checking are not implemented here. Ordinary [`crate::check_file`] retains the strict temporal refusal; its
//! explicit final/short option exposes this backend and labels TLC overrides.

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
    /// Lossless JSON trace data, including the complete false-property set.
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

/// A temporal result with total binding, exploration and validation time.
#[derive(Clone, Debug)]
pub struct Report {
    outcome: Outcome,
    options: CheckOptions,
    elapsed: Duration,
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
    /// Total wall-clock time, including independent validation.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

/// An explicit refusal, resource bound or internal inconsistency. No error
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
    /// A value could not be represented in the lossless trace document.
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
            Self::Validation { report, .. }
                if matches!(report.validation, Validation::Invalid(_)) =>
            {
                "DIVE-A0031"
            }
            Self::Validation { .. } => "DIVE-A0032",
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
            Self::Validation { report, .. } => write!(
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
    /// Conversion to the lossless trace document.
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
        if self.exploration.is_some() {
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
            let report = dive_trace::validate(&validation_model, &trace, None);
            if report.validation != Validation::Valid {
                let exploration = Some(Box::new(
                    dive_liveness::analyze::ExplorationEvidence::from(&candidate.exploration),
                ));
                return Err(Failure {
                    error: Error::Validation {
                        candidate: Box::new(candidate),
                        report,
                    },
                    stage: Stage::Validation,
                    exploration,
                    elapsed: Duration::ZERO,
                });
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
                dive_trace::validate_doc(&validation_model, &document, &config.model_values, None)
                    .0;
            if report.validation != Validation::Valid {
                let exploration = Some(Box::new(
                    dive_liveness::analyze::ExplorationEvidence::from(&candidate.exploration),
                ));
                return Err(Failure {
                    error: Error::Validation {
                        candidate: Box::new(candidate),
                        report,
                    },
                    stage: Stage::Validation,
                    exploration,
                    elapsed: Duration::ZERO,
                });
            }
            Ok(Outcome::Violation(Box::new(ValidatedCounterexample {
                candidate,
                document,
                validation: report,
            })))
        }
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
