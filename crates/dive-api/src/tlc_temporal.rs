//! Bounded pinned final-only temporal checking for Rust callers.
//!
//! This is an explicit library mode, not ordinary strict `dive check` parity.
//! It constructs a model/graph itself and independently rebinds and validates
//! every selected concrete word in memory and serialized form. Periodic TLC
//! scheduling, trace recovery aliases, initial-error replay and exact-store
//! identity remain named unsupported boundaries.
use dive_check::{CheckOptions, Workers};
use dive_config::ModelConfig;
use dive_ir::Ir;
use dive_liveness::{
    phase_graph::Limits,
    tlc_final::{self, Analysis, Candidate, Evidence},
};
use dive_model::{BindOptions, Model};
use dive_trace::{Ending, Report as ValidationReport, Trace, TraceDoc, Validation};
#[path = "tlc_temporal_admission.rs"]
mod admission;

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

/// A selected word certified by fresh independent in-memory and JSON replay.
#[derive(Clone, Debug)]
pub struct Violation {
    candidate: Candidate,
    document: TraceDoc,
    validation: ValidationReport,
}
impl Violation {
    /// Concrete candidate and actual graph/SCC/attribution evidence.
    #[must_use]
    pub fn candidate(&self) -> &Candidate {
        &self.candidate
    }
    /// Serialized replayed trace with original configured property claims.
    #[must_use]
    pub fn document(&self) -> &TraceDoc {
        &self.document
    }
    /// Actual independent JSON replay report, never a backend-supplied flag.
    #[must_use]
    pub fn validation(&self) -> &ValidationReport {
        &self.validation
    }
}
/// Result restricted to supported final-only fingerprint temporal checking.
#[derive(Clone, Debug)]
pub enum Outcome {
    /// No acceptance after all supported OOS/PEM checks on the constructed graph.
    /// The underlying fingerprint collision qualification remains applicable.
    Holds(Box<Evidence>),
    /// Exact pinned selected word independently validated twice.
    Violation(Box<Violation>),
}
/// Explicit resolved final-only report; no ordinary strict CLI verdict claim.
#[derive(Clone, Debug)]
pub struct Report {
    outcome: Outcome,
    options: CheckOptions,
    elapsed: Duration,
}
impl Report {
    /// Supported final-only outcome.
    #[must_use]
    pub fn outcome(&self) -> &Outcome {
        &self.outcome
    }
    /// Actual requested profile/seed/store/pool settings.
    #[must_use]
    pub fn options(&self) -> &CheckOptions {
        &self.options
    }
    /// This facade always checks once after complete graph construction.
    #[must_use]
    pub const fn scheduling(&self) -> &'static str {
        "final-only"
    }
    /// This facade uses pinned TLC SCC/greedy postfix selection, not short mode.
    #[must_use]
    pub const fn selection(&self) -> &'static str {
        "tlc"
    }
    /// Elapsed duration of all stages, including independent validation.
    #[must_use]
    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}
/// Deciding facade failure; never a temporal success/violation certificate.
#[derive(Clone, Debug)]
pub enum Error {
    /// Shared callback-free public IR structural admission failed.
    Context(dive_eval::ContextError),
    /// Shared callback-free structural or bound-context input admission failed.
    Input(dive_eval::EvalError),
    /// This explicit mode's configuration/profile admission failed.
    Unsupported(&'static str),
    /// Fresh model binding failed with its actual output and diagnostics.
    Binding(Box<dive_model::Failure>),
    /// Ordered execution/final checking failed with available prefix evidence.
    Analysis(Box<tlc_final::Failure>),
    /// In-memory or serialized independent replay was invalid or refused.
    Validation {
        /// Selected candidate retained for inspection, without validation credit.
        candidate: Box<Candidate>,
        /// Actual independent replay failure.
        report: ValidationReport,
    },
    /// Serialization could not construct a trace document.
    Document(String),
    /// A thread failure lost execution evidence; actual width is unknown.
    Thread(String),
}
impl Error {
    /// Stable diagnostic; does not claim complete TLC error-class parity.
    #[must_use]
    pub fn code(&self) -> &'static str {
        match self {
            Self::Context(error) => error.code(),
            Self::Input(_) => "DIVE-API-L015",
            Self::Analysis(e) => e.error.code(),
            Self::Unsupported(_) => "DIVE-API-L010",
            Self::Binding(_) => "DIVE-API-L011",
            Self::Validation { .. } => "DIVE-API-L012",
            Self::Document(_) => "DIVE-API-L013",
            Self::Thread(_) => "DIVE-API-L014",
        }
    }
}
impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Context(error) => write!(f, "final-only IR admission: {error}"),
            Self::Input(error) => write!(f, "final-only input admission: {error}"),
            Self::Unsupported(s) => write!(f, "unsupported pinned final-only check: {s}"),
            Self::Binding(e) => write!(f, "final-only binding: {e:?}"),
            Self::Analysis(e) => e.error.fmt(f),
            Self::Validation { report, .. } => write!(
                f,
                "independent final-only witness replay: {:?}",
                report.validation
            ),
            Self::Document(s) | Self::Thread(s) => write!(f, "{}: {s}", self.code()),
        }
    }
}
impl std::error::Error for Error {}
/// Available worker/evidence facts on failure; lost facts remain absent.
#[derive(Clone, Debug)]
pub struct Failure {
    /// Typed deciding failure.
    pub error: Error,
    /// Actual execution width; absent when a thread failure loses it.
    pub workers: Option<Workers>,
    /// Constructed final graph evidence if checking completed before failure.
    pub evidence: Option<Box<Evidence>>,
    /// Total elapsed duration through the failure.
    pub elapsed: Duration,
}
impl std::fmt::Display for Failure {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.error.fmt(f)
    }
}
impl std::error::Error for Failure {}
#[allow(
    clippy::unnecessary_box_returns,
    reason = "public deciding failure evidence is retained boxed"
)]
fn before(error: Error, options: &CheckOptions) -> Box<Failure> {
    Box::new(Failure {
        error,
        workers: Some(Workers {
            requested: options.workers,
            used: 0,
            sequential: false,
            reason: Some("final-only admission/binding before execution".into()),
        }),
        evidence: None,
        elapsed: Duration::ZERO,
    })
}
fn bind(ir: &Arc<Ir>, config: &ModelConfig, options: &CheckOptions) -> Result<Model, Error> {
    let host = Arc::new(dive_stdlib::host::RunHost::new(
        dive_stdlib::random::Streams::new(options.seed, dive_stdlib::random::RandomMode::Tlc),
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
#[allow(
    clippy::unnecessary_box_returns,
    reason = "public deciding failure evidence is retained boxed"
)]
fn after(error: Error, evidence: &Evidence) -> Box<Failure> {
    Box::new(Failure {
        error,
        workers: Some(evidence.execution.capture.workers.clone()),
        evidence: Some(Box::new(evidence.clone())),
        elapsed: Duration::ZERO,
    })
}
fn candidate_ending(candidate: &Candidate) -> Ending {
    if candidate.back_to + 1 == candidate.states.len() {
        Ending::Stuttering
    } else {
        Ending::Lasso {
            back_to: candidate.back_to,
        }
    }
}
fn admitted_model(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    limits: Limits,
) -> Result<Model, Box<Failure>> {
    dive_eval::validate_ir(ir).map_err(|e| before(Error::Context(e), options))?;
    if options.frontier_memory_bytes.is_some()
        || options.frontier_directory.is_some()
        || options.predecessor_memory_bytes.is_some()
        || options.predecessor_directory.is_some()
    {
        return Err(before(
            Error::Unsupported("final-only transient storage receipts are not certified"),
            options,
        ));
    }
    if config.periodic.is_some()
        || config.rl_reward.is_some()
        || !config.postconditions.is_empty()
        || !config.possible.is_empty()
    {
        return Err(before(
            Error::Unsupported("periodic/RL/postcondition/possible hooks"),
            options,
        ));
    }
    if options.profile != dive_eval::Profile::Tlc
        || options.eval != dive_eval::Options::tlc()
        || options.store != dive_store::StoreMode::Fingerprint
        || !options.tlc_fingerprints
    {
        return Err(before(
            Error::Unsupported(
                "explicit TLC fingerprint profile required; exact concrete identity is not certified",
            ),
            options,
        ));
    }
    if options.workers == 0 || options.workers > limits.workers || options.workers > 64 {
        return Err(before(
            Error::Unsupported("requested final-only worker width"),
            options,
        ));
    }
    if options.sequential {
        return Err(before(
            Error::Unsupported("explicit sequential final observer execution is not certified"),
            options,
        ));
    }
    admission::check(ir, config, limits.graph.vertices)
        .map_err(|e| before(Error::Unsupported(e), options))?;
    let model = bind(ir, config, options).map_err(|e| before(e, options))?;
    model
        .validate_context()
        .map_err(|e| before(Error::Input(e), options))?;
    if !model.sequential.is_empty() {
        return Err(before(
            Error::Unsupported(
                "model sequential fallback final observer execution is not certified",
            ),
            options,
        ));
    }
    Ok(model)
}
fn run(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    limits: Limits,
) -> Result<Outcome, Box<Failure>> {
    let model = admitted_model(ir, config, options, limits)?;
    match tlc_final::check(&model, options, limits).map_err(|mut e| {
        Box::new(Failure {
            workers: e.workers.clone(),
            evidence: e.evidence.take(),
            error: Error::Analysis(e),
            elapsed: Duration::ZERO,
        })
    })? {
        Analysis::Holds(evidence) => Ok(Outcome::Holds(Box::new(evidence))),
        Analysis::Candidate(candidate) => {
            let validation =
                bind(ir, config, options).map_err(|e| after(e, &candidate.evidence))?;
            validation
                .validate_context()
                .map_err(|e| after(Error::Input(e), &candidate.evidence))?;
            let trace = Trace {
                states: candidate.states.clone(),
                labels: vec![None; candidate.states.len()],
                ending: candidate_ending(&candidate),
                length: Some(candidate.states.len()),
                properties: candidate.properties.clone(),
            };
            let report = dive_trace::validate(&validation, &trace, None);
            if report.validation != Validation::Valid {
                return Err(after(
                    Error::Validation {
                        candidate: Box::new(candidate.clone()),
                        report,
                    },
                    &candidate.evidence,
                ));
            }
            let document = trace
                .to_doc(&validation)
                .map_err(|e| after(Error::Document(e), &candidate.evidence))?;
            let report =
                dive_trace::validate_doc(&validation, &document, &config.model_values, None).0;
            if report.validation != Validation::Valid {
                return Err(after(
                    Error::Validation {
                        candidate: Box::new(candidate.clone()),
                        report,
                    },
                    &candidate.evidence,
                ));
            }
            Ok(Outcome::Violation(Box::new(Violation {
                candidate,
                document,
                validation: report,
            })))
        }
    }
}
/// Construct and independently certify the supported pinned final-only result.
///
/// The resolved schedule is always `final-only`, selection is `tlc`, and the
/// explicit profile must be TLC fingerprint mode. Ordinary strict checking,
/// periodic schedules and exact-store model proof remain unavailable here.
/// No caller-supplied graph or validation flag is accepted.
///
/// ```
/// use std::sync::Arc;
/// use dive_api::tlc_temporal;
/// use dive_check::CheckOptions;
/// use dive_liveness::phase_graph::Limits;
/// # fn example() -> Result<(), Box<dyn std::error::Error>> {
/// let source = "---- MODULE Final ----\nVARIABLE x\nInit == x=0\nNext == UNCHANGED x\nLive == []<>(x=1)\n====\n";
/// let analysis = dive_sema::analyze_source("Final.tla", source, &Default::default());
/// let ir = Arc::new(dive_ir::lower(&analysis)?);
/// let config = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY Live\n").into_config().ok_or("invalid model configuration")?;
/// let report = tlc_temporal::check_final(&ir, &config, &CheckOptions::tlc(0, 0), Limits::default())?;
/// assert_eq!(report.scheduling(), "final-only");
/// # Ok(())
/// # }
/// ```
///
/// # Errors
/// Named unsupported configurations, actual binding/execution/limit failures,
/// refused/invalid independent validation, serialization or lost thread evidence.
pub fn check_final(
    ir: &Arc<Ir>,
    config: &ModelConfig,
    options: &CheckOptions,
    limits: Limits,
) -> Result<Report, Box<Failure>> {
    let started = Instant::now();
    let outcome = std::thread::scope(|s| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(s, || run(ir, config, options, limits))
            .map_err(|e| {
                Box::new(Failure {
                    error: Error::Thread(e.to_string()),
                    workers: None,
                    evidence: None,
                    elapsed: Duration::ZERO,
                })
            })?
            .join()
            .map_err(|_| {
                Box::new(Failure {
                    error: Error::Thread("final-only evaluation thread panicked".into()),
                    workers: None,
                    evidence: None,
                    elapsed: Duration::ZERO,
                })
            })?
    })
    .map_err(|mut e| {
        e.elapsed = started.elapsed();
        e
    })?;
    Ok(Report {
        outcome,
        options: options.clone(),
        elapsed: started.elapsed(),
    })
}
