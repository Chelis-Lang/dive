//! Bounded final temporal checking for Rust callers.
//!
//! This API checks the supported pure temporal subset and independently validates
//! every periodic counterexample before returning it as a violation. It binds a
//! fresh model for each run and for validation. It selects a deterministic short
//! lasso, including with TLC value semantics; TLC-exact selection and periodic
//! checking are not implemented here. Ordinary [`crate::check_file`] continues
//! to refuse temporal models until its report/CLI integration is complete.

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
) -> Result<Outcome, Error> {
    let model = bind(ir, config, options)?;
    match dive_liveness::analyze::analyze_final(&model, options, limits) {
        Ok(Analysis::Holds(complete)) => Ok(Outcome::Holds(Box::new(complete))),
        Ok(Analysis::Candidate(candidate)) => {
            // Rebind separately: validation cannot inspect the checker's graph
            // or depend on its mutable evaluator/host state.
            let validation_model = bind(ir, config, options)?;
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
                return Err(Error::Validation {
                    candidate: Box::new(candidate),
                    report,
                });
            }
            let document = trace.to_doc(&validation_model).map_err(Error::Document)?;
            // Validate the serialized form too; a conversion bug must not turn
            // a certified in-memory word into an incorrect user-facing trace.
            let report =
                dive_trace::validate_doc(&validation_model, &document, &config.model_values, None)
                    .0;
            if report.validation != Validation::Valid {
                return Err(Error::Validation {
                    candidate: Box::new(candidate),
                    report,
                });
            }
            Ok(Outcome::Violation(Box::new(ValidatedCounterexample {
                candidate,
                document,
                validation: report,
            })))
        }
        Err(AnalysisError::Exploration(result)) => Ok(Outcome::Safety(result)),
        Err(error) => Err(Error::Analysis(error)),
    }
}

/// Binds and checks a lowered specification's pure temporal properties after
/// complete exploration, returning only independently validated violations.
/// The run uses [`dive_liveness::analyze::Limits`] and the supplied profile,
/// store, seed and worker request. Capture currently runs inline and reports
/// its actual width. Selection is always `short`, including when `options`
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
    let started = Instant::now();
    let outcome = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(s, || run(ir, config, options, limits))
            .map_err(|e| Error::Thread(e.to_string()))?
            .join()
            .map_err(|_| Error::Thread("the evaluation thread panicked".into()))?
    })?;
    Ok(Report {
        outcome,
        options: options.clone(),
        elapsed: started.elapsed(),
    })
}
