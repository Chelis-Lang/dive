//! Model checking (`dive check`): load, bind and check a specification, and
//! report the result as a versioned JSON document or TLC-style text.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use dive_check::{CheckOptions, CheckResult, Verdict};
use dive_eval::Profile;
use dive_sema::{Options, SearchPath};
use dive_store::StoreMode;
use serde_json::{Value, json};

use crate::{AnalyzeReport, ConfigReport};

#[path = "candidate_error.rs"]
mod candidate_error;

/// Version of ordinary safety JSON documents produced by [`CheckReport::to_json`].
pub const CHECK_SCHEMA_VERSION: u32 = crate::REPORT_SCHEMA_VERSION;

/// Version of temporal JSON documents, shared with all public reports.
/// Temporal claims require their actual final or prefix execution facts.
pub const TEMPORAL_CHECK_SCHEMA_VERSION: u32 = crate::REPORT_SCHEMA_VERSION;

/// Historical native exit codes, selected by `dive check --exit-codes native`
/// and the dive profile's [`CheckReport::exit_code`]. The CLI default uses TLC's
/// deciding status independently of evaluator profile (`EC.ExitStatus`: 0,
/// 10–14, 75–77, 150, 151, 255). Codes below marked "both" keep their meaning.
pub mod exit {
    /// No error found.
    pub const SUCCESS: u8 = 0;
    /// The specification or configuration was rejected, or TLC's binding
    /// of the model failed (dive profile; TLC's 150/151 under `--compat=tlc`).
    pub const REJECTED: u8 = 1;
    /// A malformed command line or unreadable input (both).
    pub const USAGE: u8 = 2;
    /// dive refused, naming the feature, or hit a resource limit; no verdict
    /// (both).
    pub const UNSUPPORTED: u8 = 3;
    /// An internal error in dive; no verdict (both).
    pub const INTERNAL: u8 = 4;
    /// The run's budget (`--time-limit`, `--max-memory`) ran out; no verdict
    /// (both; ADR 0107).
    pub const STOPPED: u8 = 5;
    /// An invariant, property, action property or postcondition is violated.
    pub const VIOLATION: u8 = 10;
    /// A deadlock was reached.
    pub const DEADLOCK: u8 = 11;
    /// An assumption is false or failed to evaluate.
    pub const ASSUMPTION: u8 = 12;
    /// Evaluation failed: an error, a failed `Assert`, an overflow, an
    /// incompletely specified state.
    pub const EVALUATION: u8 = 13;
}

/// Process status conventions, independently of evaluator semantics.
///
/// `dive check` selects [`Self::Tlc`] by default. Existing
/// [`CheckReport::exit_code`] callers retain their profile's historical status;
/// select explicitly with [`CheckReport::exit_code_for`].
///
/// ```
/// use dive_api::{CheckReport, ExitConvention};
/// fn process_status(report: &CheckReport) -> u8 {
///     report.exit_code_for(ExitConvention::Tlc)
/// }
/// # let _ = process_status;
/// ```
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ExitConvention {
    /// Historical native statuses, including violation 10 and rejection 1.
    Native,
    /// The existing deciding TLC error-class status, without changing evaluation.
    Tlc,
}

impl ExitConvention {
    /// The CLI spelling reported in `options.exit_codes`.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Native => "native",
            Self::Tlc => "tlc",
        }
    }
}

/// The store `dive check` uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum Store {
    /// Fingerprints (the default, as in TLC).
    #[default]
    Fingerprint,
    /// Full states (`--store exact`, D-007).
    Exact,
}

/// A run-budget setting (ADR 0107): the profile's default, no limit, or an
/// explicit value.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum BudgetSetting<T> {
    /// The profile's default: for the time limit
    /// [`dive_check::budget::DEFAULT_TIME_LIMIT`] in the dive profile and none
    /// under `--compat=tlc`; for memory [`dive_check::budget::default_memory`]
    /// in both.
    #[default]
    Default,
    /// No limit (`none`).
    Unlimited,
    /// This limit.
    Explicit(T),
}

impl<T: Copy> BudgetSetting<T> {
    pub(crate) fn resolve(self, default: Option<T>) -> (Option<T>, &'static str) {
        match self {
            Self::Default => (default, if default.is_some() { "default" } else { "none" }),
            Self::Unlimited => (None, "none"),
            Self::Explicit(value) => (Some(value), "explicit"),
        }
    }
}

/// Temporal checking strategy for a request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TemporalMode {
    /// Ordinary checking. Eligible temporal requests use bounded final analysis
    /// in the TLC profile and deterministic completed-level checks in the dive
    /// profile.
    #[default]
    Ordinary,
    /// Bounded final analysis with independently validated short lassos.
    /// TLC value profiles are labeled as mixed overrides.
    FinalShort,
}

/// What to check and how.
#[derive(Clone, Debug, Default)]
pub struct CheckRequest {
    /// Explicit bounded transient disk seen-state route. None keeps the memory
    /// reference; disk uses reported inline fallback and is not a total-RAM budget.
    pub seen_disk: Option<dive_check::SeenOptions>,
    /// Opt-in encoded frontier budget per current/next queue, in bytes. This
    /// does not bound seen sets, candidates, traces, graphs or process memory.
    pub frontier_memory_bytes: Option<usize>,
    /// Parent directory for private transient frontier spill files. Requires
    /// [`Self::frontier_memory_bytes`]; a directory alone is rejected.
    pub frontier_directory: Option<PathBuf>,
    /// Separate retained predecessor-record capacity budget for safety BFS.
    /// Zero spills immediately; omitted preserves the in-memory counterpart.
    /// Reconstruction buffers and all other search storage are outside it.
    pub predecessor_memory_bytes: Option<usize>,
    /// Parent of private transient log directories. Requires a budget; omitted
    /// uses the system temporary directory. Files are removed before certification.
    pub predecessor_directory: Option<PathBuf>,
    /// The root module file.
    pub file: PathBuf,
    /// The configuration; `<file stem>.cfg` beside it by default, as in TLC.
    pub config: Option<PathBuf>,
    /// Module directories searched before the standard modules.
    pub library_paths: Vec<PathBuf>,
    /// Module directories searched after the standard modules.
    pub module_paths: Vec<PathBuf>,
    /// `--compat=tlc`.
    pub compat_tlc: bool,
    /// Whole-run symmetry/liveness policy; profile default when omitted.
    /// TLC-equivalent: `Tlc`, currently refused for quotient temporal checking.
    pub symmetry_liveness: Option<dive_model::SymmetryLiveness>,
    /// `--action-composition off|tlc|tla` (D-022); `None` is the profile's
    /// setting: `tla` by default, `off` under `--compat=tlc` (TLC without
    /// `-Dtlc2.tool.impl.Tool.cdot=true`).
    pub action_composition: Option<dive_eval::ActionComposition>,
    /// The store.
    pub store: Store,
    /// `--seed`; a fresh seed otherwise (always reported).
    pub seed: Option<i64>,
    /// `--core-sample N` (`--engine core`): after the search, compare the
    /// successors of `N` sampled expanded states with the reference
    /// evaluator's (`docs/developer/core/08-correctness.md` §4). Diagnostic
    /// only: every result is unchanged. `None` samples nothing.
    pub core_sample: Option<usize>,
    /// `--fp`; TLC picks one at random under `--compat=tlc`, dive uses 0.
    pub fp: Option<usize>,
    /// `--workers`; `None` is the profile's default (TLC's 1 under
    /// `--compat=tlc`, the available parallelism otherwise).
    pub workers: Option<usize>,
    /// TLC's `-deadlock`: do not check for deadlock.
    pub no_deadlock: bool,
    /// `--sequential`: every level inline with one logical worker, as
    /// sequential mode does (ADR 0015 §6). Results are identical.
    pub sequential: bool,
    /// Test-only chaos scheduling (`--chaos SEED`, or `DIVE_CHAOS`); see
    /// `dive_check::CheckOptions::chaos`. Results must be identical.
    pub chaos: Option<u64>,
    /// Temporal checking strategy. `FinalShort` selects bounded final-only
    /// analysis and labels TLC profiles `tlc+overrides`.
    /// `Ordinary` selects final analysis for eligible TLC-profile temporal roles
    /// and deterministic completed-level checks for eligible dive-profile roles.
    /// Unsupported combinations retain named refusals.
    pub temporal: TemporalMode,
    /// Completed-level schedule for ordinary dive-profile temporal checking
    /// (D-014). The default is the documented [`PeriodicSchedule::DEFAULT`];
    /// TLC-profile and explicit final requests ignore it and check once at the end.
    pub periodic: PeriodicSchedule,
    /// `--time-limit` (ADR 0107): wall-clock time for the whole check.
    pub time_limit: BudgetSetting<std::time::Duration>,
    /// `--max-memory` (ADR 0107): the whole process's resident-memory
    /// ceiling, in bytes.
    pub max_memory: BudgetSetting<u64>,
    /// Maximum admitted distinct in-model identities; `None` is TLC-equivalent.
    pub max_states: Option<u64>,
    /// Maximum BFS discovery depth (Init is depth one); `None` is TLC-equivalent.
    pub max_depth: Option<u64>,
}

/// Completed-level schedule for ordinary dive-profile temporal checking.
pub use dive_liveness::analyze::periodic::Schedule as PeriodicSchedule;

/// The phase a check ended in.
#[derive(Debug)]
enum Phase {
    /// Resource exhaustion observed before model binding begins.
    BudgetStopped(dive_check::budget::Stopped),
    /// The specification was rejected by the front end.
    SpecRejected(Box<AnalyzeReport>),
    /// The configuration was rejected.
    ConfigRejected(Box<ConfigReport>),
    /// The specification could not be lowered (a refusal).
    Unlowered(String),
    /// The checker ran.
    Checked(Box<CheckResult>),
    /// Independently validated bounded temporal analysis.
    Temporal(Box<crate::temporal::Report>),
    /// No temporal verdict: a refusal, limit, evaluation or internal failure.
    TemporalError(Box<crate::temporal::Failure>),
}

/// The outcome of `dive check`.
#[derive(Debug)]
pub struct CheckReport {
    checkpoint_request: Option<dive_check::checkpoint::Request>,
    checkpoint_receipt: Option<dive_check::checkpoint::Receipt>,
    file: String,
    config: String,
    profile: Profile,
    opts: CheckOptions,
    seed_given: bool,
    symmetry_liveness_explicit: bool,
    fp_given: bool,
    workers_requested: usize,
    temporal: TemporalMode,
    /// Where the time and memory limits came from: `default`, `explicit`
    /// or `none`.
    budget_sources: (&'static str, &'static str),
    memory_requested_source: &'static str,
    memory_samples: Option<dive_check::budget::MemorySamples>,
    temporal_configuration: crate::temporal::Configuration,
    phase: Phase,
    /// The core engine's run, when `--engine=core` ran the check.
    core: Option<Box<crate::core_engine::CoreRun>>,
    /// Untrusted original source context captured while this run owns the IR.
    next_error_endpoint: Option<dive_trace::NextStateEndpoint>,
    /// Untrusted candidate-site observation; fresh replay proves its identities.
    candidate_error_endpoint: Option<dive_trace::CandidateStateEndpoint>,
    view_error_endpoint: Option<dive_trace::ViewSourceEndpoint>,
    /// Observed partial candidate captured while its original IR is owned.
    partial_terminal_endpoint: Option<dive_trace::PartialNextEndpoint>,
    /// The engine requested.
    engine: crate::Engine,
    /// Why `--engine=auto` ran the reference engine.
    fallback: Option<crate::core_engine::Fallback>,
}

fn admit_storage_request(req: &CheckRequest) -> Result<(), crate::ApiError> {
    if let Some(disk) = &req.seen_disk {
        disk.limits
            .validate()
            .map_err(|error| crate::ApiError::Rejected {
                message: error.to_string(),
            })?;
        if req.temporal != TemporalMode::Ordinary {
            return Err(crate::ApiError::Unsupported {
                feature:
                    "disk seen-state retention with temporal/capture checking is not implemented"
                        .into(),
            });
        }
    }
    if req.frontier_directory.is_some() && req.frontier_memory_bytes.is_none() {
        return Err(crate::ApiError::Rejected {
            message: "DIVE-A0100: frontier_directory requires frontier_memory_bytes".into(),
        });
    }
    if req.predecessor_directory.is_some() && req.predecessor_memory_bytes.is_none() {
        return Err(crate::ApiError::Rejected {
            message: "DIVE-A0101: predecessor directory requires predecessor memory budget".into(),
        });
    }
    if req.predecessor_memory_bytes.is_some() && req.temporal != TemporalMode::Ordinary {
        return Err(crate::ApiError::Unsupported {
            feature: "predecessor spill with bounded temporal checking is not implemented".into(),
        });
    }
    Ok(())
}

fn admit_checkpoint_storage(
    req: &CheckRequest,
    engine: crate::Engine,
) -> Result<(), crate::ApiError> {
    if req.seen_disk.is_some() && engine == crate::Engine::Reference {
        return Err(crate::ApiError::Unsupported {
            feature:
                "disk seen-state retention with Reference checkpoint/recovery is not implemented"
                    .into(),
        });
    }
    Ok(())
}

/// Loads, binds and checks the request.
///
/// # Errors
/// [`crate::ApiError`] if the root file or the configuration cannot be read.
pub fn check_file(req: &CheckRequest) -> Result<CheckReport, crate::ApiError> {
    check_file_in(req, None, false, crate::Engine::Reference, None)
}

/// Loads and checks files as [`check_file`] does, with the given engine.
///
/// [`crate::Engine::Core`] runs the M0 spike of the new execution core: the
/// default profile only, one worker, safety only. What it does not
/// implement is an `unsupported` verdict naming the construct; it never falls
/// back to the reference engine.
///
/// ```no_run
/// use dive_api::{CheckRequest, Engine, check_file_engine};
/// let req = CheckRequest { file: "Spec.tla".into(), ..CheckRequest::default() };
/// let report = check_file_engine(&req, Engine::Core).unwrap();
/// println!("{}", report.to_json()["engine"]);
/// ```
///
/// # Errors
/// As [`check_file`].
pub fn check_file_engine(
    req: &CheckRequest,
    engine: crate::Engine,
) -> Result<CheckReport, crate::ApiError> {
    check_file_in(req, None, false, engine, None)
}

/// Checks with the core engine under explicit strategy switches, as
/// `DIVE_CORE_OPT` gives them (for example `-memo,-delta-hash`); the
/// environment variable is ignored. Every switch must leave every result
/// unchanged.
///
/// # Errors
/// As [`check_file`].
pub fn check_file_core_opts(
    req: &CheckRequest,
    switches: &str,
) -> Result<CheckReport, crate::ApiError> {
    check_file_in(req, None, false, crate::Engine::Core, Some(switches))
}

/// Loads and checks files with TLC's configuration-first input error priority.
///
/// Used by the `dive tlc` and `tlc` checking entrypoints after resolving their
/// filenames. Unlike [`check_file`], this reads and parses the configuration
/// before analyzing the module: an unreadable or malformed configuration wins
/// when both inputs are invalid. Request defaults and path spellings are unchanged.
///
/// ```no_run
/// use dive_api::{CheckRequest, check_file_tlc_inputs};
/// let request = CheckRequest {
///     file: "Counter.tla".into(),
///     compat_tlc: true,
///     ..CheckRequest::default()
/// };
/// let report = check_file_tlc_inputs(&request)?;
/// # Ok::<(), dive_api::ApiError>(())
/// ```
///
/// # Errors
/// [`crate::ApiError`] if the configuration or root file cannot be read.
pub fn check_file_tlc_inputs(req: &CheckRequest) -> Result<CheckReport, crate::ApiError> {
    check_file_in(req, None, true, crate::Engine::Reference, None)
}

/// Runs the first native pure-safety checkpoint/recovery slice. It refuses
/// temporal combinations before loading files. Existing frontier/predecessor
/// budgets configure transient storage; native archive work is separately bounded
/// and excluded from both budgets. Recovery uses
/// saved seed/fingerprint defaults when not explicitly supplied; profile/store
/// and all semantic options must match. Actual workers are reported separately.
///
/// ```no_run
/// use dive_api::{CheckRequest, check_file_checkpoint, checkpoint};
/// let model = CheckRequest { file: "Counter.tla".into(), ..CheckRequest::default() };
/// let native = checkpoint::Request {
///     path: "counter.cp".into(),
///     action: checkpoint::Action::PauseAfterLevels(2),
///     counters: checkpoint::Counters::Tlc,
///     config_source_sha256: None,
/// };
/// let report = check_file_checkpoint(&model, &native)?;
/// assert_eq!(report.to_json()["options"]["checkpoint"]["action"], "pause");
/// # Ok::<(), dive_api::ApiError>(())
/// ```
///
/// # Errors
/// File I/O or malformed native request/metadata, without a model verdict.
pub fn check_file_checkpoint(
    req: &CheckRequest,
    checkpoint: &dive_check::checkpoint::Request,
) -> Result<CheckReport, crate::ApiError> {
    if req.seen_disk.is_some() {
        return Err(crate::ApiError::Unsupported {
            feature:
                "disk seen-state retention with Reference checkpoint/recovery is not implemented"
                    .into(),
        });
    }
    check_file_checkpoint_engine(req, checkpoint, crate::Engine::Reference)
}

/// Runs a checkpoint transaction on an explicitly selected whole backend.
/// `Auto` pauses on Reference and recovers on the backend encoded in the archive.
///
/// ```no_run
/// use dive_api::{CheckRequest, Engine, checkpoint, check_file_checkpoint_engine};
/// let model=CheckRequest{file:"Counter.tla".into(),..CheckRequest::default()};
/// let checkpoint=checkpoint::Request{path:"counter.cp".into(),action:checkpoint::Action::PauseAfterLevels(2),counters:checkpoint::Counters::Retain,config_source_sha256:None};
/// let report=check_file_checkpoint_engine(&model,&checkpoint,Engine::Core)?;
/// # Ok::<(),dive_api::ApiError>(())
/// ```
///
/// # Errors
/// File loading, backend mismatch, unsupported request or archive framing failure.
pub fn check_file_checkpoint_engine(
    req: &CheckRequest,
    checkpoint: &dive_check::checkpoint::Request,
    engine: crate::Engine,
) -> Result<CheckReport, crate::ApiError> {
    if req.temporal != TemporalMode::Ordinary {
        return Err(crate::ApiError::Unsupported {
            feature: "native checkpoint with temporal checking is not implemented".into(),
        });
    }
    check_file_in(req, Some(checkpoint), false, engine, None)
}

#[allow(
    clippy::too_many_lines,
    reason = "single ordered loader dispatch for ordinary and native recovery admission"
)]
fn check_file_in(
    req: &CheckRequest,
    checkpoint: Option<&dive_check::checkpoint::Request>,
    config_first: bool,
    engine: crate::Engine,
    core_switches: Option<&str>,
) -> Result<CheckReport, crate::ApiError> {
    admit_storage_request(req)?;
    let profile = if req.compat_tlc {
        Profile::Tlc
    } else {
        Profile::Dive
    };
    let mut selected_engine = engine;
    if let Some(request) = checkpoint {
        if engine == crate::Engine::Auto
            && matches!(
                request.action,
                dive_check::checkpoint::Action::PauseAfterLevels(_)
            )
        {
            selected_engine = crate::Engine::Reference;
        }
        // A statically known unsupported storage/backend combination must not
        // open a recovery archive or load the model to discover that refusal.
        admit_checkpoint_storage(req, selected_engine)?;
    }
    let recovered_defaults = if let Some(request) = checkpoint
        && request.action == dive_check::checkpoint::Action::Recover
    {
        let (encoded, seed, fp) =
            crate::core_checkpoint::metadata(&request.path).map_err(|message| {
                crate::ApiError::Rejected {
                    message: format!("DIVE-S0400: {message}"),
                }
            })?;
        if engine != crate::Engine::Auto && engine != encoded {
            return Err(crate::ApiError::Rejected {
                message: format!(
                    "DIVE-S0400: archive backend {} does not match requested {}",
                    encoded.name(),
                    engine.name()
                ),
            });
        }
        selected_engine = encoded;
        // Auto recovery needs the encoded backend first, then rejects an
        // unsupported disk policy before model loading or evaluation.
        admit_checkpoint_storage(req, selected_engine)?;
        Some((seed, fp))
    } else {
        None
    };
    let seed = req
        .seed
        .or(recovered_defaults.map(|p| p.0))
        .unwrap_or_else(dive_stdlib::random::fresh_seed);
    let fp = req
        .fp
        .or(recovered_defaults.map(|p| p.1))
        .unwrap_or_else(|| {
            if req.compat_tlc {
                // TLC: a random polynomial per run unless `-fp` is given
                // (`TLC.java`); reported either way.
                usize::try_from(seed.unsigned_abs() % 131).unwrap_or(0)
            } else {
                0
            }
        });
    let workers_requested = req.workers.unwrap_or_else(|| {
        if req.compat_tlc {
            1
        } else {
            std::thread::available_parallelism().map_or(1, std::num::NonZero::get)
        }
    });
    let mut opts = if req.compat_tlc {
        CheckOptions::tlc(fp, seed)
    } else {
        let mut o = CheckOptions::dive(seed);
        o.fp_index = fp;
        o
    };
    opts.store = match req.store {
        Store::Fingerprint => StoreMode::Fingerprint,
        Store::Exact => StoreMode::Exact,
    };
    opts.no_deadlock = req.no_deadlock;
    if let Some(policy) = req.symmetry_liveness {
        opts.symmetry_liveness = policy;
    }
    if let Some(mode) = req.action_composition {
        opts.eval.action_composition = mode;
    }
    opts.seen_disk.clone_from(&req.seen_disk);
    opts.frontier_memory_bytes = req.frontier_memory_bytes;
    opts.frontier_directory.clone_from(&req.frontier_directory);
    opts.workers = workers_requested;
    opts.sequential = req.sequential;
    opts.chaos = req.chaos;
    opts.predecessor_memory_bytes = req.predecessor_memory_bytes;
    opts.max_states = req.max_states;
    opts.max_depth = req.max_depth;
    opts.predecessor_directory
        .clone_from(&req.predecessor_directory);
    let (time, time_source) = req.time_limit.resolve(if req.compat_tlc {
        None
    } else {
        Some(dive_check::budget::DEFAULT_TIME_LIMIT)
    });
    let memory_resolution = resolve_memory(req.max_memory, dive_check::budget::default_memory);
    let memory_requested_source = match req.max_memory {
        BudgetSetting::Default => "default",
        BudgetSetting::Explicit(_) => "explicit",
        BudgetSetting::Unlimited => "none",
    };
    let (memory, memory_source) = memory_resolution.unwrap_or((None, "unavailable"));
    opts.budget = dive_check::budget::Budget::new(time, memory);
    let cfg_path = req
        .config
        .clone()
        .unwrap_or_else(|| req.file.with_extension("cfg"));
    let mut report = CheckReport {
        checkpoint_request: checkpoint.cloned(),
        checkpoint_receipt: None,
        file: req.file.display().to_string(),
        config: cfg_path.display().to_string(),
        profile,
        opts,
        seed_given: req.seed.is_some(),
        symmetry_liveness_explicit: req.symmetry_liveness.is_some(),
        fp_given: req.fp.is_some(),
        workers_requested,
        temporal: req.temporal,
        budget_sources: (time_source, memory_source),
        memory_requested_source,
        memory_samples: None,
        temporal_configuration: crate::temporal::file_configuration(),
        phase: Phase::Unlowered(String::new()),
        core: None,
        next_error_endpoint: None,
        candidate_error_endpoint: None,
        view_error_endpoint: None,
        partial_terminal_endpoint: None,
        engine,
        fallback: None,
    };
    // TLC's Spec constructor parses ModelConfig before creating SpecProcessor
    // (tlaplus 94d0c50). Keep the file API's existing module-first order.
    let config = if config_first {
        let cfg = crate::parse_config_file(&cfg_path)?;
        if !cfg.accepted() {
            report.phase = Phase::ConfigRejected(Box::new(cfg));
            return Ok(report);
        }
        Some(cfg)
    } else {
        None
    };
    let search = Options {
        search: SearchPath {
            library_paths: req.library_paths.clone(),
            after_standard: req.module_paths.clone(),
        },
        ..Options::default()
    };
    let analysis =
        dive_sema::analyze_file(&req.file, &search).map_err(|error| crate::ApiError::Io {
            path: report.file.clone(),
            error,
        })?;
    if !analysis.accepted() {
        report.phase = Phase::SpecRejected(Box::new(AnalyzeReport::from_analysis(
            report.file.clone(),
            analysis,
        )));
        return Ok(report);
    }
    let cfg = match config {
        Some(cfg) => cfg,
        None => crate::parse_config_file(&cfg_path)?,
    };
    let Some(model) = cfg.config().cloned() else {
        report.phase = Phase::ConfigRejected(Box::new(cfg));
        return Ok(report);
    };
    let ir = match dive_ir::lower(&analysis) {
        Ok(ir) => Arc::new(ir),
        Err(e) => {
            report.phase = Phase::Unlowered(format!("lowering the specification: {e:?}"));
            return Ok(report);
        }
    };
    // One clock and one ceiling for every stage of the check itself; loading,
    // parsing and lowering above are not budgeted.
    memory_resolution.map_err(crate::ApiError::BudgetMeasurement)?;
    let checking_started = std::time::Instant::now();
    let (mut watchdog, armed) =
        dive_check::budget::arm(&report.opts).map_err(|error| match error {
            dive_check::budget::StartError::Measurement(error) => {
                crate::ApiError::BudgetMeasurement(error)
            }
            error @ dive_check::budget::StartError::Thread(_) => {
                crate::ApiError::BudgetStartup(error)
            }
        })?;
    let armed = armed.into_owned();
    report.phase = if let Some(exhausted) = armed
        .budget
        .poll()
        .map_err(crate::ApiError::BudgetMeasurement)?
    {
        Phase::BudgetStopped(exhausted.during(dive_check::budget::Phase::Startup))
    } else if let Some(request) = checkpoint {
        let mut request = request.clone();
        request.config_source_sha256 = Some(dive_check::checkpoint::config_source_hash(
            cfg.source_bytes(),
        ));
        let native = if selected_engine == crate::Engine::Core {
            let (native, run) = crate::core_checkpoint::check(
                &ir,
                &model,
                &armed,
                &request,
                profile == Profile::Tlc,
            );
            report.core = Some(Box::new(run));
            native
        } else {
            if engine == crate::Engine::Auto {
                report.fallback=Some(crate::core_engine::Fallback{stage:"static",reason:"Core checkpoint Auto pause admission is not yet proved; recovery selects the archive backend".into()});
            }
            dive_check::checkpoint::check(&ir, &model, &armed, &request)
        };
        report.checkpoint_receipt = native.receipt;
        Phase::Checked(Box::new(native.check))
    } else if engine == crate::Engine::Core {
        let (result, run) = crate::core_engine::check(
            &ir,
            &model,
            &armed,
            req.compat_tlc,
            core_switches,
            req.core_sample,
            core_route(req, &armed),
        );
        report.core = Some(Box::new(run));
        result.into()
    } else if engine == crate::Engine::Auto && req.seen_disk.is_some() {
        report.fallback = Some(crate::core_engine::Fallback {
            stage: "static",
            reason: "whole-backend Auto admission for Core disk membership is not yet proved"
                .into(),
        });
        check_backend(&ir, &model, &armed, req.temporal, req.periodic)
    } else if engine == crate::Engine::Auto {
        let (result, run) = crate::core_engine::check(
            &ir,
            &model,
            &armed,
            req.compat_tlc,
            core_switches,
            req.core_sample,
            core_route(req, &armed),
        );
        if let Some(fallback) = crate::core_engine::Fallback::of(&result, &run) {
            // The whole check again, on the reference engine; the core's
            // partial results (counts, output, metrics) are discarded.
            report.fallback = Some(fallback);
            drop(result);
            check_backend(&ir, &model, &armed, req.temporal, req.periodic)
        } else {
            report.core = Some(Box::new(run));
            result.into()
        }
    } else {
        check_backend(&ir, &model, &armed, req.temporal, req.periodic)
    };
    if report.core.is_some() && req.temporal == TemporalMode::Ordinary {
        report.temporal_configuration = crate::temporal::core_file_configuration();
    }
    if let Phase::Checked(result) = &mut report.phase {
        certify_next_receiver(&ir, &model, &armed, result);
    }
    if let Phase::Checked(result) = &report.phase
        && let Verdict::EvaluationError {
            site: dive_check::ErrorSite::Next,
            cause,
            reported,
            code,
            verified,
            ..
        } = &result.verdict
    {
        report.next_error_endpoint = if *verified && cause.exception_kind().is_some() {
            dive_trace::NextStateEndpoint::from_ordered_error(&ir, cause, *reported, *code)
        } else {
            dive_trace::NextStateEndpoint::from_error(&ir, cause)
        };
    }
    if let Phase::Checked(result) = &report.phase
        && let Verdict::EvaluationError {
            site,
            cause,
            reported,
            code,
            verified: true,
            ..
        } = &result.verdict
        && matches!(
            (cause.exception_kind(), *code),
            (Some(dive_eval::ExceptionKind::ToolRuntime), 2103)
                | (
                    Some(dive_eval::ExceptionKind::EvalException),
                    2132 | 2179 | 2178
                )
        )
    {
        let site = match site {
            dive_check::ErrorSite::Constraint => Some(dive_trace::CandidateErrorSite::Constraint),
            dive_check::ErrorSite::ActionConstraint => {
                Some(dive_trace::CandidateErrorSite::ActionConstraint)
            }
            dive_check::ErrorSite::ImpliedAction => {
                Some(dive_trace::CandidateErrorSite::ImpliedAction)
            }
            _ => None,
        };
        if let Some(site) = site {
            report.candidate_error_endpoint =
                dive_trace::CandidateStateEndpoint::from_error(&ir, cause, site, *reported, *code);
        }
    }
    if let Phase::Checked(result) = &report.phase
        && let Verdict::EvaluationError {
            site: dive_check::ErrorSite::View,
            cause,
            reported,
            code,
            verified: true,
            ..
        } = &result.verdict
        && matches!(
            (cause.exception_kind(), *code),
            (Some(dive_eval::ExceptionKind::ToolRuntime), 2103)
                | (
                    Some(dive_eval::ExceptionKind::EvalException),
                    2132 | 2179 | 2178
                )
        )
    {
        report.view_error_endpoint =
            dive_trace::ViewSourceEndpoint::from_error(&ir, cause, *reported, *code);
    }
    candidate_error::certify(&mut report, &ir, &model, &armed);
    candidate_error::certify_view(&mut report, &ir, &model, &armed);
    candidate_error::certify_invariant(&mut report, &ir, &model, &armed);
    if let Phase::Checked(result) = &report.phase
        && let Verdict::NotCompletelySpecified {
            initial,
            initial_candidate,
            ..
        } = &result.verdict
        && let Some(partial) = if *initial {
            initial_candidate.as_ref()
        } else {
            result
                .counterexample
                .as_ref()
                .and_then(|cx| cx.states.last())
                .and_then(|state| state.partial.as_ref())
        }
    {
        let mut assignments = serde_json::Map::new();
        let mut unassigned = Vec::new();
        let mut encodable = true;
        for (name, value) in result.var_names.iter().zip(&partial.values) {
            if *initial && !matches!(armed.budget.poll(), Ok(None)) {
                encodable = false;
                break;
            }
            if let Some(value) = value {
                // Init evidence must not force compound/deferred diagnostics
                // after the actual controlled deciding event. Scalars retain
                // the report's existing primitive serialization units.
                if *initial && (value.as_set().is_some() || value.as_fcn().is_some()) {
                    encodable = false;
                    break;
                }
                if let Ok(value) = dive_trace::value_json::to_json(value, report.opts.eval.ctx) {
                    assignments.insert(name.clone(), value);
                } else {
                    encodable = false;
                    break;
                }
            } else {
                unassigned.push(name.clone());
            }
        }
        if encodable && partial.values.len() == result.var_names.len() {
            report.partial_terminal_endpoint = if *initial {
                dive_trace::PartialStateEndpoint::from_initial_candidate(
                    &ir,
                    partial.expression,
                    assignments,
                    unassigned,
                )
            } else {
                dive_trace::PartialStateEndpoint::from_candidate(
                    &ir,
                    partial.expression,
                    assignments,
                    unassigned,
                )
            };
        }
    }
    if report.partial_terminal_endpoint.is_none()
        && let Phase::Checked(result) = &mut report.phase
        && matches!(
            result.verdict,
            Verdict::NotCompletelySpecified { initial: true, .. }
        )
    {
        result.notes.push("Init partial error witness remains unconfirmed: its observed source/bindings were unavailable, its compound assignment encoding is unsupported, or the controlling budget stopped evidence capture.".into());
    }
    if let Some(watchdog) = &mut watchdog {
        let finished = watchdog.finish();
        report.finish_budget(finished, checking_started.elapsed());
    }
    report.memory_samples = armed.budget.memory_samples();
    drop(watchdog);
    Ok(report)
}

/// The core's temporal route for this request: ordinary checking only,
/// with the default profile's completed-level schedule or, under the TLC
/// profile, the final check alone (as `check_backend` routes it).
fn core_route(req: &CheckRequest, options: &CheckOptions) -> Option<crate::core_temporal::Route> {
    (req.temporal == TemporalMode::Ordinary)
        .then(|| (options.profile == dive_eval::Profile::Dive).then_some(req.periodic))
}

impl From<crate::core_engine::CoreOutcome> for Phase {
    fn from(outcome: crate::core_engine::CoreOutcome) -> Self {
        match outcome {
            crate::core_engine::CoreOutcome::Checked(r) => Self::Checked(r),
            crate::core_engine::CoreOutcome::Temporal(r) => Self::Temporal(r),
            crate::core_engine::CoreOutcome::TemporalFailure(f) => Self::TemporalError(f),
        }
    }
}

pub(crate) fn resolve_memory(
    setting: BudgetSetting<u64>,
    default: impl FnOnce() -> Option<u64>,
) -> Result<(Option<u64>, &'static str), dive_check::budget::MeasurementError> {
    match setting {
        BudgetSetting::Default => default()
            .filter(|bytes| *bytes != 0)
            .map(|bytes| (Some(bytes), "default"))
            .ok_or_else(dive_check::budget::MeasurementError::default_ceiling_unavailable),
        BudgetSetting::Explicit(bytes) => Ok((Some(bytes), "explicit")),
        BudgetSetting::Unlimited => Ok((None, "none")),
    }
}

/// Admit a diagnostic wrapper only after fresh source-bound word validation.
/// The producer's typed receiver observation is a request, never a certificate.
fn certify_next_receiver(
    ir: &Arc<dive_ir::Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    result: &mut CheckResult,
) {
    let Verdict::EvaluationError {
        site: dive_check::ErrorSite::Next,
        cause,
        reported,
        verified: false,
        ..
    } = &result.verdict
    else {
        return;
    };
    if !cause.value_method().is_some_and(|receiver| {
        receiver.method() == dive_eval::ValueMethod::FunctionMaterialize
            && receiver.source().is_none()
    }) {
        return;
    }
    let Some(path) = result
        .counterexample
        .as_ref()
        .filter(|path| path.states.len() == 1)
    else {
        return;
    };
    let Some(endpoint) =
        dive_trace::NextStateEndpoint::from_ordered_error(ir, cause, *reported, 2147)
    else {
        return;
    };
    let request = trace_doc(
        result,
        &path.states,
        options.eval.ctx,
        Some(&endpoint),
        None,
        None,
        None,
    )
    .and_then(|value| dive_trace::TraceDoc::from_json(&value));
    let Ok(document) = request else {
        return;
    };
    let control = match dive_check::budget::EvaluationControl::new(
        &options.budget,
        dive_check::budget::Phase::Validation,
    ) {
        Ok(control) => control,
        Err(error) => {
            result
                .notes
                .push(format!("Next receiver proof refused: {error}"));
            return;
        }
    };
    let recipe = dive_model::Recipe::new(
        ir.clone(),
        config.clone(),
        options.eval,
        options.no_deadlock,
    )
    .with_symmetry_liveness(options.symmetry_liveness);
    let session = dive_trace::Session::new(recipe, None);
    match session.read_controlled(&document, control) {
        Ok((report, _)) if report.validation == dive_trace::Validation::Valid => {
            if let Verdict::EvaluationError { code, name, verified, .. } = &mut result.verdict {
                *code = 2147;
                *name = "TLC_FINGERPRINT_EXCEPTION";
                *verified = true;
            }
        }
        Ok((report, _)) => result.notes.push(format!(
            "Next function receiver diagnostic lifetime remains unproved: {:?}",
            report.validation
        )),
        Err(error) => result.notes.push(format!(
            "Next function receiver diagnostic proof stopped; the original cause, class and counts are retained: {error}"
        )),
    }
}

fn check_backend(
    ir: &Arc<dive_ir::Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    temporal: TemporalMode,
    periodic: PeriodicSchedule,
) -> Phase {
    if temporal == TemporalMode::Ordinary
        && let Some(result) = crate::temporal::check_ordinary_final(ir, config, options, periodic)
    {
        return match result {
            Ok(report) => match report.outcome() {
                crate::temporal::Outcome::Safety(result) => Phase::Checked(result.clone()),
                _ => Phase::Temporal(Box::new(report)),
            },
            Err(failure) => Phase::TemporalError(Box::new(failure)),
        };
    }
    if temporal == TemporalMode::FinalShort {
        let configuration = crate::temporal::file_configuration();
        return match crate::temporal::check_final_with_policies(
            ir,
            config,
            options,
            configuration.limits,
            configuration.acceptance,
            configuration.graph_labels,
        ) {
            Ok(r) => match r.outcome() {
                crate::temporal::Outcome::Safety(s) => Phase::Checked(s.clone()),
                _ => Phase::Temporal(Box::new(r)),
            },
            Err(e) => Phase::TemporalError(Box::new(e)),
        };
    }
    let mut result = dive_check::check(ir, config, options);
    if temporal == TemporalMode::Ordinary
        && let dive_check::Verdict::Unsupported { feature } = &mut result.verdict
        && feature.contains("temporal properties (liveness checking)")
        && let Some(reason) = dive_check::final_roles::route_exclusion(ir, config, options)
    {
        feature.push_str("; temporal checking does not yet support ");
        feature.push_str(reason);
    }
    Phase::Checked(Box::new(result))
}

impl CheckReport {
    fn finish_budget(
        &mut self,
        finished: Result<
            Option<dive_check::budget::Exhausted>,
            dive_check::budget::MeasurementError,
        >,
        elapsed: Duration,
    ) {
        match &mut self.phase {
            Phase::Checked(result) => {
                result.elapsed = elapsed;
                dive_check::budget::finish_result(
                    result,
                    finished,
                    dive_check::budget::Phase::Finalization,
                );
            }
            Phase::Temporal(report) => {
                if let Some(failure) = crate::temporal::finish_report(report, finished, elapsed) {
                    self.phase = Phase::TemporalError(Box::new(failure));
                }
            }
            // An earlier genuine error keeps its classification.
            _ => {}
        }
    }
    fn temporal_schema(&self) -> bool {
        self.temporal == TemporalMode::FinalShort
            || matches!(&self.phase, Phase::Temporal(_) | Phase::TemporalError(_))
    }

    /// The safety checker's result, including a safety stop in final/short
    /// checking. Temporal holds/violations use [`Self::temporal_result`].
    #[must_use]
    pub fn result(&self) -> Option<&CheckResult> {
        match &self.phase {
            Phase::Checked(r) => Some(r),
            _ => None,
        }
    }

    /// Separate original initial-state diagnostic context. This is not a formal
    /// counterexample; its named cause and deciding replay need independent proof.
    #[must_use]
    pub fn diagnostic_witness(&self) -> Option<&dive_check::DiagnosticWitness> {
        self.result()
            .and_then(|result| result.diagnostic_witness.as_ref())
    }

    /// The validated temporal result, when final/short checking ran. Safety
    /// stops are exposed through [`Self::result`] instead.
    #[must_use]
    pub fn temporal_result(&self) -> Option<&crate::temporal::Report> {
        if let Phase::Temporal(r) = &self.phase {
            Some(r)
        } else {
            None
        }
    }

    /// The verdict's name in the JSON document.
    #[must_use]
    pub fn verdict(&self) -> &'static str {
        match &self.phase {
            Phase::BudgetStopped(_) => "stopped",
            Phase::SpecRejected(_) | Phase::ConfigRejected(_) => "rejected",
            Phase::Unlowered(_) => "unsupported",
            Phase::Checked(r) => r.verdict.kind(),
            Phase::Temporal(r) => match r.outcome() {
                crate::temporal::Outcome::Holds(_) | crate::temporal::Outcome::NativeHolds(_) => {
                    "success"
                }
                crate::temporal::Outcome::Violation(_)
                | crate::temporal::Outcome::PrefixViolation(_)
                | crate::temporal::Outcome::NativeViolation(_)
                | crate::temporal::Outcome::NativePrefixViolation(_) => "liveness-violation",
                crate::temporal::Outcome::Safety(_) => "internal-error",
            },
            Phase::TemporalError(e) => temporal_failure_kind(e),
        }
    }

    /// The historical API profile's exit code (see [`exit`]).
    ///
    /// This remains native for the dive profile and TLC for the TLC profile.
    /// The CLI's independent default is selected with [`Self::exit_code_for`].
    #[must_use]
    pub fn exit_code(&self) -> u8 {
        self.exit_code_for(if self.profile == Profile::Tlc {
            ExitConvention::Tlc
        } else {
            ExitConvention::Native
        })
    }

    /// Selects a process status without changing the evaluator's profile.
    ///
    /// TLC statuses come from the deciding class, not its reported message.
    /// Refusal/resource 3 and internal error 4 are preserved in both conventions.
    /// This does not establish parity of an unverified class or model result.
    #[must_use]
    pub fn exit_code_for(&self, convention: ExitConvention) -> u8 {
        let tlc = convention == ExitConvention::Tlc;
        match &self.phase {
            Phase::BudgetStopped(_) => exit::STOPPED,
            Phase::SpecRejected(_) => {
                if tlc {
                    150
                } else {
                    exit::REJECTED
                }
            }
            Phase::ConfigRejected(_) => {
                if tlc {
                    151
                } else {
                    exit::REJECTED
                }
            }
            Phase::Unlowered(_) => exit::UNSUPPORTED,
            Phase::Temporal(r) => match r.outcome() {
                crate::temporal::Outcome::Holds(_) | crate::temporal::Outcome::NativeHolds(_) => {
                    exit::SUCCESS
                }
                crate::temporal::Outcome::Violation(_)
                | crate::temporal::Outcome::PrefixViolation(_)
                | crate::temporal::Outcome::NativeViolation(_)
                | crate::temporal::Outcome::NativePrefixViolation(_) => {
                    if tlc {
                        13
                    } else {
                        exit::VIOLATION
                    }
                }
                crate::temporal::Outcome::Safety(_) => exit::INTERNAL,
            },
            Phase::TemporalError(e) => match temporal_failure_kind(e) {
                "internal-error" => exit::INTERNAL,
                "stopped" => exit::STOPPED,
                "model-error" => {
                    if tlc {
                        if let crate::temporal::Error::Binding(failure) = e.error() {
                            failure
                                .error
                                .code()
                                .map_or(exit::INTERNAL, dive_check::tlc_exit_status)
                        } else {
                            exit::INTERNAL
                        }
                    } else {
                        exit::REJECTED
                    }
                }
                "evaluation-error" => {
                    if tlc {
                        if temporal_startup_tautology(e) {
                            dive_check::tlc_exit_status(
                                dive_model::Ec::TlcLiveFormulaTautology.code(),
                            )
                        } else {
                            temporal_evaluation(e).map_or(exit::INTERNAL, |error| {
                                dive_check::tlc_exit_status(error.ec().code())
                            })
                        }
                    } else {
                        exit::EVALUATION
                    }
                }
                _ => exit::UNSUPPORTED,
            },
            Phase::Checked(r) => {
                let v = &r.verdict;
                match v {
                    Verdict::Unsupported { .. } | Verdict::ResourceLimit { .. } => {
                        exit::UNSUPPORTED
                    }
                    Verdict::Stopped(_) => exit::STOPPED,
                    Verdict::InternalError { .. } | Verdict::BudgetMeasurement { .. } => {
                        exit::INTERNAL
                    }
                    _ if tlc => v.tlc().map_or(exit::INTERNAL, |c| c.exit),
                    Verdict::Success => exit::SUCCESS,
                    Verdict::InvariantViolated { .. }
                    | Verdict::PropertyViolatedInitially { .. }
                    | Verdict::ActionPropertyViolated { .. }
                    | Verdict::PostconditionFalse { .. } => exit::VIOLATION,
                    Verdict::Deadlock => exit::DEADLOCK,
                    Verdict::AssumptionFalse { .. }
                    | Verdict::EvaluationError {
                        site: dive_check::ErrorSite::Assumption,
                        ..
                    } => exit::ASSUMPTION,
                    Verdict::ModelError { .. } => exit::REJECTED,
                    Verdict::EvaluationError { .. } | Verdict::NotCompletelySpecified { .. } => {
                        exit::EVALUATION
                    }
                }
            }
        }
    }

    fn profile_name(&self) -> &'static str {
        match self.profile {
            Profile::Tlc
                if self.temporal == TemporalMode::FinalShort
                    || self.opts.eval.action_composition == dive_eval::ActionComposition::Tla
                    || self.checkpoint_request.as_ref().is_some_and(|r| {
                        r.counters == dive_check::checkpoint::Counters::Retain
                    })
                    // TLC has no time limit (D-026). A memory ceiling is
                    // TLC's `-Xmx` counterpart, a host resource (ADR 0107).
                    || self.budget_sources.0 == "explicit"
                    || self.opts.max_states.is_some() || self.opts.max_depth.is_some()
                    || self.opts.symmetry_liveness != dive_model::SymmetryLiveness::Tlc =>
            {
                "tlc+overrides"
            }
            Profile::Tlc => "tlc",
            Profile::Dive => "dive",
        }
    }

    fn profile_for_exit_convention(&self, convention: ExitConvention) -> &'static str {
        if self.profile == Profile::Tlc && convention == ExitConvention::Native {
            "tlc+overrides"
        } else {
            self.profile_name()
        }
    }

    /// Reports an explicitly selected physical status and its convention.
    ///
    /// The underlying evaluator settings and deciding class are unchanged.
    /// Native statuses with TLC evaluation are labeled `tlc+overrides`.
    /// [`Self::to_json`] retains its historical API document.
    #[must_use]
    pub fn to_json_with_exit_convention(&self, convention: ExitConvention) -> Value {
        let mut document = self.to_json();
        document["exit_code"] = json!(self.exit_code_for(convention));
        document["options"]["exit_codes"] = json!(convention.as_str());
        document["options"]["semantic_profile"] = json!(if self.profile == Profile::Tlc {
            "tlc"
        } else {
            "dive"
        });
        document["options"]["profile"] = json!(self.profile_for_exit_convention(convention));
        document
    }

    /// Human output with the resolved physical status and compatibility label.
    /// [`Self::render_human`] retains its historical API output.
    #[must_use]
    pub fn render_human_with_exit_convention(&self, convention: ExitConvention) -> String {
        use std::fmt::Write as _;
        let mut output = self.render_human_profile(self.profile_for_exit_convention(convention));
        let _ = writeln!(
            output,
            "Exit codes: {} (process status {})",
            convention.as_str(),
            self.exit_code_for(convention)
        );
        output
    }

    /// The `options` block: profile, options and seed (ADR 0010).
    fn symmetry_liveness(&self) -> Option<&dive_model::SymmetryLivenessResolution> {
        match &self.phase {
            Phase::Checked(result) => result.symmetry_liveness.as_ref(),
            Phase::Temporal(report) => report.symmetry_liveness(),
            Phase::TemporalError(failure) => failure.symmetry_liveness(),
            _ => None,
        }
    }

    fn analysis_configuration(&self) -> crate::temporal::Configuration {
        match &self.phase {
            Phase::Temporal(report) => report.configuration(),
            Phase::TemporalError(failure) => failure.configuration(),
            _ => None,
        }
        .unwrap_or(self.temporal_configuration)
    }

    fn options_json(&self) -> Value {
        let o = &self.opts;
        let e = &o.eval;
        let int = match e.ctx.int_mode {
            dive_value::IntMode::Tlc => "tlc",
            dive_value::IntMode::Big => "big",
        };
        let order = match e.ctx.order {
            dive_value::ValueOrder::Tlc => "tlc",
            dive_value::ValueOrder::Canonical => "canonical",
        };
        let explore = match e.exploration {
            dive_eval::ExplorationOrder::Tlc => "tlc",
            dive_eval::ExplorationOrder::Value => "value",
        };
        let comparisons = match e.comparisons {
            dive_eval::Comparisons::Tlc => "tlc",
            dive_eval::Comparisons::Tla => "tla",
        };
        let fingerprint = (o.store == StoreMode::Fingerprint).then(|| {
            json!({
                "polynomial_index": o.fp_index,
                "polynomial_source": if self.fp_given { "flag" } else if self.checkpoint_request.as_ref().is_some_and(|r| r.action == dive_check::checkpoint::Action::Recover) { "checkpoint" } else if self.profile == Profile::Tlc { "random" } else { "default" },
                "scheme": if o.tlc_fingerprints { "tlc" } else { "dive" },
            })
        });
        let mut options = json!({
            "profile": self.profile_name(),
            "symmetry_liveness": o.symmetry_liveness.as_str(),
            "symmetry_liveness_source": if self.symmetry_liveness_explicit { "explicit" } else { "profile-default" },
            "int": int,
            "choose_order": order,
            "explore_order": explore,
            "comparisons": comparisons,
            "warnings": e.warnings,
            "action_composition": match e.action_composition {
                dive_eval::ActionComposition::Off => "off",
                dive_eval::ActionComposition::Tlc => "tlc",
                dive_eval::ActionComposition::Tla => "tla",
            },
            "store": o.store.name(),
            "fingerprint": fingerprint,
            "seed": o.seed,
            "seed_source": if self.seed_given { "flag" } else if self.checkpoint_request.as_ref().is_some_and(|r| r.action == dive_check::checkpoint::Action::Recover) { "checkpoint" } else { "fresh" },
            "deadlock_flag": o.no_deadlock,
            "workers": self.workers_json(),
            "frontier_memory_bytes_per_queue": o.frontier_memory_bytes,
            "frontier_record_bytes_limit": o.frontier_memory_bytes.map(|_| dive_store::spill::MAX_RECORD_BYTES),
            "frontier_codec_nodes_per_value_limit": o.frontier_memory_bytes.filter(|_| self.core.is_none()).map(|_| dive_value::codec::FRONTIER_MAX_NODES),
            "predecessor_memory_bytes": o.predecessor_memory_bytes,
            "predecessor_path_steps_limit": o.predecessor_memory_bytes.map(|_| dive_store::predecessor::MAX_PATH_STEPS),
            "budget": {
                "time_limit_ms": o.budget.time.map(|t| u64::try_from(t.as_millis()).unwrap_or(u64::MAX)),
                "time_limit_source": self.budget_sources.0,
                "max_memory_bytes": o.budget.memory,
                "max_memory_source": self.budget_sources.1,
                "max_memory_requested_source": self.memory_requested_source,
                "max_states": o.max_states,
                "max_depth": o.max_depth,
                "scope": "the check after loading: evaluator-backed fresh model binding after structural configuration/evaluator construction, safety search, graph capture, temporal analysis and fresh native temporal-candidate validation including its rebinding, and producer ASSUME using existing amortized loop checkpoints; structural binding, independent validation ASSUME, post-binding symmetry preparation, serialization, quiet value/bulk/native units, deferred providers retained by other evaluators and diagnostic reconstruction after a deciding event are not interrupted; legacy TLC/session and standalone validation are not controlled; memory of the whole process",
            },
        });
        if let Some(disk) = &o.seen_disk {
            options["seen_disk"] = json!({"entries_limit": disk.limits.entries, "key_bytes_limit": disk.limits.key_bytes, "probe_steps_limit": disk.limits.probe_steps, "summary_pairs_limit": disk.limits.summary_pairs, "directory": disk.directory, "execution": "inline-fallback", "resident_key_collection_bytes": 0, "total_memory_budget": false});
        }
        if self.temporal_schema() {
            options["lncheck"] = json!(self.temporal_result().map_or_else(
                || if let Phase::TemporalError(failure) = &self.phase {
                    if failure.periodic().is_some() {
                        "deterministic"
                    } else {
                        "final"
                    }
                } else {
                    "final"
                },
                crate::temporal::Report::lncheck
            ));
            options["lasso"] = json!("short");
            options["temporal_bounds"] = temporal_limits_json(self.analysis_configuration());
            options["tlc_exact_temporal"] = json!(false);
        }
        if let Some(request) = &self.checkpoint_request {
            options["checkpoint"] = json!({
                "path": request.path,
                "action": match request.action { dive_check::checkpoint::Action::Recover => "recover", dive_check::checkpoint::Action::PauseAfterLevels(_) => "pause" },
                "pause_after_levels": match request.action { dive_check::checkpoint::Action::PauseAfterLevels(n) => Some(n), dive_check::checkpoint::Action::Recover => None },
                "counters": request.counters,
                "max_payload_bytes": dive_check::checkpoint::MAX_PAYLOAD_BYTES,
                "max_states": dive_check::checkpoint::MAX_STATES,
                "max_state_bytes": self.core.is_none().then_some(dive_check::checkpoint::MAX_STATE_BYTES),
                "backend": if self.core.is_some() {"core"} else {"reference"},
                "archive_bounds_scope": "archive admission only; ordinary search, Store, codec workspaces and process memory are separate",
                "max_level": dive_check::checkpoint::MAX_LEVEL,
                "max_counter": dive_check::checkpoint::MAX_COUNTER,
                "trusted_local_archive": true,
                "independent_certificate": false,
                "parallel_recovery": true,
            });
        }
        options
    }

    /// The workers requested and used: `{ requested, used, sequential,
    /// reason, chaos }`. These, with the timings, are the only fields of the
    /// document that may differ between worker counts.
    fn workers_json(&self) -> Value {
        if let Phase::TemporalError(failure) = &self.phase {
            return match failure.workers(self.workers_requested) {
                Some(w) => json!({
                    "requested": w.requested, "used": w.used,
                    "sequential": w.sequential, "reason": w.reason,
                    "chaos": self.opts.chaos,
                }),
                None => json!({
                    "requested": self.workers_requested, "used": null,
                    "sequential": null, "reason": "execution evidence unavailable",
                    "chaos": self.opts.chaos,
                }),
            };
        }
        let w = self
            .result()
            .map(|r| r.workers.clone())
            .or_else(|| {
                self.temporal_result()
                    .and_then(temporal_evidence)
                    .map(|c| c.workers)
            })
            .unwrap_or_else(|| dive_check::Workers {
                requested: self.workers_requested,
                used: 0,
                sequential: false,
                reason: Some("no search ran".to_owned()),
            });
        json!({
            "requested": w.requested,
            "used": w.used,
            "sequential": w.sequential,
            "reason": w.reason,
            "chaos": self.opts.chaos,
        })
    }

    /// The workers used, for the human header.
    fn workers_used(&self) -> String {
        match &self.phase {
            Phase::Checked(r) => r.workers.used.to_string(),
            Phase::Temporal(r) => temporal_evidence(r)
                .map_or(0, |c| c.workers.used)
                .to_string(),
            Phase::TemporalError(failure) => failure
                .workers(self.workers_requested)
                .map_or_else(|| "unknown".into(), |w| w.used.to_string()),
            _ => "0".into(),
        }
    }

    /// The JSON document (schema [`CHECK_SCHEMA_VERSION`]). `check_kind` names
    /// the safety or temporal request/report family; it does not assert that
    /// checking ran or that capture completed. Final/short safety failures
    /// retain the temporal family and their actual execution evidence:
    ///
    /// ```text
    /// { "schema_version": 3, "command": "check", "file", "config",
    ///   "verdict": "success" | "invariant-violation" | "property-violation"
    ///            | "action-property-violation" | "deadlock" | "assumption-failure"
    ///            | "postcondition-failure" | "assertion-failure" | "overflow"
    ///            | "evaluation-error" | "incomplete-state" | "model-error"
    ///            | "rejected" | "unsupported" | "resource-limit" | "stopped"
    ///            | "internal-error",
    ///   "property": string | null, "message": string | null,
    ///   "error_site": string | null,
    ///   "tlc": { "code", "name", "exit_status", "reported", "verified" } | null,
    ///   "statistics": { "generated", "distinct", "left_on_queue", "depth",
    ///                   "initial_generated", "reported_by_tlc" } | null,
    ///   "counterexample": { "length", "ending": "finite",
    ///                       "states": [ { "index", "label", "action",
    ///                                     "values": { var: TLC JSON }, "text" } ],
    ///                       "trace": dive-trace document | null } | null,
    ///   "diagnostic_witness": { "trace": native-v2 named cause document }, // optional
    ///   "collision": { "store", "scheme", "optimistic", "actual", "assumption" } | null,
    ///   "output": [ string ], "warnings": [ { "code", "message" } ],
    ///   "diagnostics": [ ... ], "notes": [ string ],
    ///   "stopped": { "resource": "time" | "memory" | "states" | "depth", "limit", "observed", "elapsed_ms",
    ///                "unit": "ms" | "bytes" | "states" | "levels", "phase", "hint" }   (only when stopped),
    ///   "options": { "profile", "int", "choose_order", "explore_order",
    ///                "comparisons", "warnings", "store", "fingerprint",
    ///                "seed", "seed_source", "deadlock_flag",
    ///                "budget": { "time_limit_ms", "time_limit_source",
    ///                            "max_memory_bytes", "max_memory_source", "max_states", "max_depth", "scope" },
    ///                "workers": { "requested", "used", "sequential", "reason",
    ///                             "chaos" } },
    ///   "elapsed_ms" }
    /// ```
    ///
    /// `tlc` is how TLC classifies the result: `code` decides TLC's exit
    /// status, and `reported` is the code of the error message TLC prints
    /// first (an invariant that fails to evaluate is reported as
    /// `TLC_INVARIANT_EVALUATION_FAILED`, for example); `verified: false`
    /// means dive does not yet reproduce TLC's error class at that site and
    /// does not claim it. `reported_by_tlc` is false where TLC prints no statistics
    /// (a stop while generating initial states). Explicit final-only schema 3
    /// also declares the schedule/selection, fixed entry/work bounds and actual
    /// failure stage. Periodic traces include all configured property names and
    /// a stuttering ending or one-based lasso target; independent replay precedes
    /// any private-certified temporal violation.
    #[must_use]
    #[allow(clippy::too_many_lines, reason = "one document")]
    pub fn to_json(&self) -> Value {
        let mut doc = json!({
            "schema_version": CHECK_SCHEMA_VERSION,
            "check_kind": if self.temporal_schema() { "temporal" } else { "safety" },
            "command": "check",
            "file": self.file,
            "config": self.config,
            "verdict": self.verdict(),
            "property": Value::Null,
            "message": Value::Null,
            "error_site": Value::Null,
            "tlc": Value::Null,
            "statistics": Value::Null,
            "counterexample": Value::Null,
            "collision": Value::Null,
            "frontier_storage": Value::Null,
            "output": [],
            "warnings": [],
            "diagnostics": [],
            "notes": [],
            "options": self.options_json(),
            "symmetry_liveness": self.symmetry_liveness().map(symmetry_liveness_json),
            "elapsed_ms": 0,
        });
        let samples = self.memory_samples.unwrap_or_default();
        doc["resources"] = json!({"memory": {
            "metric": if cfg!(target_os = "linux") { "rss+swap" } else if cfg!(target_os = "macos") { "rss" } else { "unavailable" },
            "status": if self.opts.budget.memory.is_none() { if self.budget_sources.1 == "unavailable" { "unavailable" } else { "disabled" } } else if samples.unavailable.is_some() { "unavailable" } else if self.memory_samples.is_some() { "sampled" } else { "not-started" },
            "samples": samples.count, "observed_max_bytes": samples.maximum,
            "initial_sampled": samples.initial, "final_sampled": samples.final_sample,
            "unavailable_at": samples.unavailable.map(dive_check::budget::SamplePoint::name),
            "scope": "sampled process memory, not an allocation-time hard cap or a measurement of the true peak; excursions between observations are possible"
        }});
        match &self.phase {
            Phase::BudgetStopped(stopped) => {
                doc["message"] = json!(stopped.to_string());
                doc["stopped"] = stopped_json(stopped);
                doc["elapsed_ms"] = json!(stopped.elapsed_ms);
                doc["resources"]["checking_started"] = json!(false);
                if self.temporal_schema() {
                    doc["execution"] = json!({"stage": "check-startup", "capture_completed": false, "started": false});
                }
            }
            Phase::SpecRejected(a) => {
                doc["diagnostics"] = a.to_json()["diagnostics"].clone();
                // TLC reports every spec SANY rejects, in its parse or its
                // semantic phase, as `TLC_PARSING_FAILED` (3002), exit 150.
                // Verified against captured runs of the pinned TLC: harness
                // fixtures `ParseError` and `SemanticError`, and the goldens
                // of the probes `M1Syntax` (a syntax error) and `M2Undefined`
                // (an undefined name) in `conformance/corpus/probes/`. Like
                // every other verdict's class, it is reported in both
                // profiles.
                doc["tlc"] = json!({"code": 3002, "name": "TLC_PARSING_FAILED", "exit_status": 150, "verified": true});
            }
            Phase::ConfigRejected(c) => {
                doc["diagnostics"] = c.to_json()["diagnostics"].clone();
            }
            Phase::Unlowered(why) => doc["message"] = Value::from(why.as_str()),
            Phase::Temporal(r) => {
                temporal_json(&mut doc, r, self.opts.tlc_fingerprints);
                if self.temporal == TemporalMode::Ordinary {
                    let violation = matches!(
                        r.outcome(),
                        crate::temporal::Outcome::Violation(_)
                            | crate::temporal::Outcome::PrefixViolation(_)
                            | crate::temporal::Outcome::NativeViolation(_)
                            | crate::temporal::Outcome::NativePrefixViolation(_)
                    );
                    doc["execution"]["exhausted"] =
                        json!(r.periodic().is_none_or(|checks| checks.exhausted));
                    doc["tlc"] = if violation {
                        json!({"code":2116,"name":"TLC_TEMPORAL_PROPERTY_VIOLATED","exit_status":13,"verified":true})
                    } else {
                        json!({"code":2193,"name":"TLC_SUCCESS","exit_status":0,"verified":true})
                    };
                }
            }
            Phase::TemporalError(e) => {
                if let Some(progress) = e.validation_progress() {
                    doc["resources"]["validation"] = json!({
                        "checks_passed": progress.checks_passed,
                        "state": progress.state,
                        "check": progress.check.map(dive_trace::Check::name),
                    });
                }
                doc["message"] = json!(e.to_string());
                doc["diagnostics"] = json!([crate::diagnostics::temporal_failure(e)]);
                doc["execution"] =
                    json!({"stage": e.stage().name(), "capture_completed": e.capture_completed()});
                if e.stage() == crate::temporal::Stage::Binding
                    && e.exploration().is_none()
                    && !matches!(e.error(), crate::temporal::Error::Binding(_))
                {
                    doc["resources"]["checking_started"] = json!(false);
                }
                if let Some(checks) = e.periodic() {
                    doc["periodic"] = periodic_json(checks);
                }
                doc["elapsed_ms"] =
                    json!(u64::try_from(e.elapsed().as_millis()).unwrap_or(u64::MAX));
                if let crate::temporal::Error::Binding(failure) = e.error()
                    && let Some(class) = Verdict::from(&failure.error).tlc()
                {
                    doc["tlc"] = json!({
                        "code": class.code,
                        "name": class.name,
                        "exit_status": class.exit,
                        "reported": class.reported,
                        "verified": class.verified,
                    });
                } else if temporal_startup_tautology(e) {
                    let class = dive_model::Ec::TlcLiveFormulaTautology;
                    doc["tlc"] = json!({
                        "code": class.code(), "name": class.name(),
                        "reported": class.code(), "verified": true,
                        "exit_status": dive_check::tlc_exit_status(class.code()),
                    });
                } else if let Some(error) = temporal_evaluation(e) {
                    let code = error.ec().code();
                    doc["tlc"] = json!({
                        "code": code,
                        "name": error.ec().name(),
                        "exit_status": dive_check::tlc_exit_status(code),
                        "verified": if e.capture_row_error().is_some() { true } else if temporal_action_evaluation(e).is_some() {
                            error.ec() == dive_eval::Ec::General && !error.is_eval_exception()
                        } else { error.ec() == dive_eval::Ec::ExpectedValue },
                    });
                }
                if temporal_action_evaluation(e).is_some() {
                    doc["error_site"] = json!("temporal-action");
                }
                if let crate::temporal::Error::Analysis(
                    dive_liveness::analyze::AnalysisError::Stopped(stopped),
                ) = e.error()
                {
                    doc["stopped"] = stopped_json(stopped);
                }
                if let Some(capture) = e.exploration() {
                    temporal_evidence_json(&mut doc, capture, self.opts.tlc_fingerprints);
                    doc["notes"] = json!([
                        "Temporal checking failed; capture_completed records whether these actual exploration statistics describe completion. No temporal verdict was produced."
                    ]);
                } else {
                    doc["warnings"] = json!(
                        e.binding_output()
                            .iter()
                            .filter_map(|m| {
                                if let dive_model::Message::Warning { code, message } = m {
                                    Some(json!({"code": code.code(), "message": message}))
                                } else {
                                    None
                                }
                            })
                            .collect::<Vec<_>>()
                    );
                    doc["output"] = json!(
                        e.binding_output()
                            .iter()
                            .filter_map(|m| match m {
                                dive_model::Message::Print(text) => Some(text.clone()),
                                dive_model::Message::Warning { .. } => None,
                            })
                            .collect::<Vec<_>>()
                    );
                }
                if let Some((error, document, _)) = e.capture_row_error() {
                    doc["message"] = json!(error.to_string());
                    doc["error_site"] =
                        json!(if e.state_evaluation().is_some_and(|state| state.site()
                            == dive_liveness::analyze::StateEvaluationSite::CaptureSourceRow)
                        {
                            "temporal-source-row"
                        } else {
                            "temporal-state-row"
                        });
                    doc["tlc"]["reported"] = json!(error.ec().code());
                    doc["statistics"]["reported_by_tlc"] = json!(true);
                    doc["counterexample"] = json!({
                        "length": document.states.len(), "ending": "finite",
                        "states": document.states.iter().enumerate().map(|(i, values)| json!({"index":i+1,"label": if i==0 {Some("<Initial predicate>")} else {None},"action":null,"values":values,"text":null})).collect::<Vec<_>>(),
                        "trace": document.to_json(), "validation": "valid",
                    });
                    doc["notes"] = json!([
                        "The finite behavior and first ordered temporal demand error were independently reconstructed and validated on a fresh source-bound runtime. No BFS counters, lasso or violated-property claim is certified by endpoint validation."
                    ]);
                }
            }
            Phase::Checked(r) => {
                if let Some(storage) = &r.seen {
                    let facts = |s: dive_store::seen::Stats| json!({ "physical_entries": s.physical_entries, "expected_disk_bytes": s.disk_bytes, "resident_key_collection_bytes": s.resident_key_bytes, "poisoned": s.poisoned, "cleanup_completed": s.cleanup_completed });
                    doc["seen_storage"] = json!({
                        "mode": "fallible-disk", "admitted_keys": storage.admitted_keys,
                        "scope": "seen-key/offset collections only; supplied keys, state values, frontiers, predecessors, candidates, traces, graphs, OS page cache and native archives are outside these bounds",
                        "before_cleanup": storage.before_cleanup.map(facts), "after_cleanup": storage.after_cleanup.map(facts),
                        "first_failure": storage.first_failure.as_ref().map(|failure| json!({"phase": format!("{:?}", failure.phase), "operation": format!("{:?}", failure.error.operation), "code": failure.error.code(), "message": failure.error.message, "cleanup_failure": failure.error.cleanup_failure, "generated": failure.generated, "predecessor": failure.predecessor, "successor_ordinal": failure.successor_ordinal})),
                        "deciding_event_before_failure": storage.deciding_event_before_failure.as_ref().map(|v| format!("{v:?}")),
                        "replay": storage.replay.as_ref().map(|r| json!({"original_error": format!("{:?}", r.original_error), "predecessor": r.predecessor, "stopped_successor_ordinal": r.stopped_successor_ordinal, "generated": r.generated, "admitted_keys": r.admitted_keys, "queued": r.queued, "unproved_print_output": r.unproved_print_output, "halted_by": r.halted_by.as_ref().map(|v| format!("{v:?}"))})),
                        "crash_durable": false, "checkpoint_import_supported": false,
                        "checkpoint_key_reconstitution": self.core.is_some() && self.checkpoint_request.is_some(),
                    });
                }
                doc["frontier_storage"] = json!({
                    "mode": if r.frontier.memory_bytes_per_queue.is_some() { "encoded-spill" } else { "memory-reference" },
                    "scope": if self.core.is_some() && r.frontier.memory_bytes_per_queue.is_some() {
                        "encoded current and next frontier capacities observed at queue operations only; retained live Store and construction history, decoded chunks, seen sets, candidates, arenas, traces, allocator workspace and native archives are not measured or bounded"
                    } else { "encoded current and next frontier queues only; legacy value-frontier heap, candidates, seen sets, traces, graphs and native archive work are not measured or bounded" },
                    "memory_bytes_per_queue": r.frontier.memory_bytes_per_queue,
                    "max_record_bytes": r.frontier.max_record_bytes,
                    "max_codec_nodes_per_value": r.frontier.memory_bytes_per_queue.filter(|_| self.core.is_none()).map(|_| dive_value::codec::FRONTIER_MAX_NODES),
                    "peak_retained_bytes": r.frontier.peak_retained_bytes,
                    "retained_bytes": r.frontier.retained_bytes,
                    "committed_spill_bytes": r.frontier.spilled_bytes,
                    "write_accounting_complete": r.frontier.failures.is_empty(),
                    "failures": r.frontier.failures,
                    "search_event_before_storage_failure": r.frontier.deciding_event_before_failure,
                });
                if self.temporal != TemporalMode::Ordinary {
                    let stage = if r.binding_model_error
                        || matches!(&r.verdict, Verdict::Stopped(stop) if stop.phase == dive_check::budget::Phase::Binding)
                    {
                        "binding"
                    } else {
                        "capture"
                    };
                    doc["execution"] = json!({"stage": stage, "capture_completed": false});
                }
                if self.opts.predecessor_memory_bytes.is_some() {
                    let p = &r.predecessor;
                    doc["predecessor_storage"] = json!({
                        "scope": "predecessor-record-capacity-only",
                        "retained_bytes_before_cleanup": p.storage.retained_bytes,
                        "peak_retained_bytes": p.storage.peak_retained_bytes,
                        "confirmed_write_bytes": p.storage.written_bytes,
                        "committed_records": p.storage.records,
                        "spilled": p.storage.spilled,
                        "write_accounting_complete": p.failures.is_empty(),
                        "cleanup_completed": p.cleanup_completed,
                        "failures": p.failures,
                        "deciding_event_before_failure": p.deciding_event_before_failure,
                        "excludes": ["seen-set", "frontiers", "candidate-vectors", "graphs", "reconstructed-paths", "decoded-states", "allocator-workspace", "native-archive-work"]
                    });
                }
                let ctx = self.opts.eval.ctx;
                let v = &r.verdict;
                doc["property"] = v.property().map_or(Value::Null, Value::from);
                doc["message"] = match v {
                    Verdict::AssumptionFalse { message, .. }
                    | Verdict::EvaluationError { message, .. }
                    | Verdict::ModelError { message, .. }
                    | Verdict::PostconditionFalse { message, .. }
                    | Verdict::NotCompletelySpecified { message, .. }
                    | Verdict::ResourceLimit { message }
                    | Verdict::InternalError { message } => Value::from(message.as_str()),
                    Verdict::Unsupported { feature } => Value::from(feature.as_str()),
                    Verdict::Stopped(stopped) => Value::from(stopped.to_string()),
                    Verdict::BudgetMeasurement { error } => Value::from(error.to_string()),
                    _ => Value::Null,
                };
                if let Verdict::BudgetMeasurement { error } = v {
                    doc["diagnostics"] = json!([{"code": dive_check::budget::MEASUREMENT_FAILURE_CODE, "message": error.to_string(), "cause": error.to_string(), "span": null, "help": crate::diagnostics::explain(dive_check::budget::MEASUREMENT_FAILURE_CODE).map(|entry| entry.remediation)}]);
                }
                if let Verdict::Stopped(stopped) = v {
                    doc["stopped"] = stopped_json(stopped);
                }
                if let Verdict::EvaluationError { site, .. } = v {
                    doc["error_site"] = Value::from(site.name());
                }
                if let Some(c) = v.tlc() {
                    doc["tlc"] = json!({
                        "code": c.code,
                        "name": c.name,
                        "exit_status": c.exit,
                        "reported": c.reported,
                        "verified": c.verified,
                    });
                }
                if let Some(s) = &r.stats {
                    doc["statistics"] = json!({
                        "generated": s.generated,
                        "distinct": s.distinct,
                        "left_on_queue": s.left_on_queue,
                        "depth": s.depth,
                        "initial_generated": s.initial_generated,
                        // A stop's statistics are progress, not TLC's report.
                        "reported_by_tlc": r.tlc_prints_stats
                            && !matches!(r.verdict, Verdict::Stopped(_) | Verdict::BudgetMeasurement { .. }),
                    });
                }
                if let Some(cx) = &r.counterexample {
                    let states: Vec<Value> = cx
                        .states
                        .iter()
                        .enumerate()
                        .map(|(i, s)| {
                            let values: serde_json::Map<String, Value> = r
                                .tlc_var_order
                                .iter()
                                .filter_map(|&k| {
                                    let name = r.var_names.get(k)?.clone();
                                    let v = s.values.get(k)?;
                                    let j = dive_value::json::to_tlc_json(v, ctx)
                                        .ok()
                                        .and_then(|t| serde_json::from_str(&t).ok())
                                        .unwrap_or(Value::Null);
                                    Some((name, j))
                                })
                                .collect();
                            json!({
                                "index": i + 1,
                                "label": s.label,
                                "action": s.action,
                                "values": values,
                                "text": dive_check::format_trace_state(r, s, ctx),
                            })
                        })
                        .collect();
                    let transported = trace_doc(
                        r,
                        &cx.states,
                        ctx,
                        self.next_error_endpoint.as_ref(),
                        self.partial_terminal_endpoint.as_ref(),
                        self.candidate_error_endpoint.as_ref(),
                        self.view_error_endpoint.as_ref(),
                    );
                    doc["counterexample"] = json!({
                        "length": states.len(),
                        "ending": "finite",
                        "states": states,
                        "trace": transported.unwrap_or(Value::Null),
                    });
                }
                if let Some(witness) = &r.diagnostic_witness {
                    let transported = trace_doc(r, witness.states(), ctx, None, None, None, None);
                    doc["diagnostic_witness"] =
                        json!({"trace": transported.unwrap_or(Value::Null)});
                }
                if matches!(
                    r.verdict,
                    Verdict::NotCompletelySpecified { initial: true, .. }
                ) && let Some(endpoint) = &self.partial_terminal_endpoint
                {
                    let witness = dive_trace::TraceDoc {
                        variables: r.var_names.clone(),
                        states: Vec::new(),
                        labels: Vec::new(),
                        ending: Some(dive_trace::Ending::PartialInit {
                            endpoint: endpoint.clone(),
                        }),
                        length: Some(1),
                        source: dive_trace::Source::Dive,
                        properties: Vec::new(),
                    };
                    doc["error_witness"] = witness.to_json();
                }
                if let Some(c) = &r.collision {
                    doc["collision"] = json!({
                        "store": "fingerprint",
                        "scheme": if r.tlc_fingerprints { "tlc" } else { "dive" },
                        "optimistic": c.optimistic,
                        "actual": c.actual,
                        "assumption": "fingerprints behave as random 64-bit values (TLC's assumption too)",
                    });
                }
                doc["output"] = json!(r.output);
                doc["warnings"] = r
                    .warnings
                    .iter()
                    .map(|(c, m)| json!({"code": c, "message": m}))
                    .collect();
                doc["notes"] = json!(r.notes);
                doc["elapsed_ms"] = json!(u64::try_from(r.elapsed.as_millis()).unwrap_or(u64::MAX));
            }
        }
        if self.checkpoint_request.is_some() {
            doc["checkpoint"] = json!(self.checkpoint_receipt);
        }
        let measurement_failure = match &self.phase {
            Phase::Checked(result) => matches!(result.verdict, Verdict::BudgetMeasurement { .. }),
            Phase::TemporalError(failure) => {
                matches!(
                    failure.error(),
                    crate::temporal::Error::Analysis(
                        dive_liveness::analyze::AnalysisError::BudgetMeasurement(_)
                    )
                ) || matches!(failure.error(), crate::temporal::Error::Analysis(dive_liveness::analyze::AnalysisError::Exploration(result)) if matches!(result.verdict, Verdict::BudgetMeasurement { .. }))
            }
            _ => false,
        };
        if (self.verdict() == "stopped" || measurement_failure) && doc["statistics"].is_object() {
            doc["statistics"]["exhaustive"] = json!(false);
        }
        if let Some(core) = &self.core {
            doc["engine"] = json!(crate::Engine::Core.name());
            doc["core"] = json!({"disabled": core.opts, "metrics": core.metrics});
            if !core.sample.is_null() {
                doc["core"]["sample"] = core.sample.clone();
            }
        } else if self.fallback.is_some() {
            doc["engine"] = json!(crate::Engine::Reference.name());
        }
        if self.engine == crate::Engine::Auto {
            doc["engine_requested"] = json!(crate::Engine::Auto.name());
            doc["engine_fallback"] = self.fallback.as_ref().map_or(
                Value::Null,
                |f| json!({"stage": f.stage, "reason": f.reason}),
            );
        }
        doc
    }

    /// The TLC-style text report (see `dive_check::render_human`), preceded
    /// by the run's options and the `Print` output.
    #[must_use]
    pub fn render_human(&self) -> String {
        self.render_human_profile(self.profile_name())
    }

    #[allow(clippy::too_many_lines, reason = "one complete human check report")]
    fn render_human_profile(&self, profile: &str) -> String {
        use std::fmt::Write as _;
        let mut out = String::new();
        let o = &self.opts;
        let fp = if o.store == StoreMode::Fingerprint {
            format!(" (fp {})", o.fp_index)
        } else {
            String::new()
        };
        let _ = writeln!(
            out,
            "dive check {}: profile {}, store {}{fp}, seed {}, {} worker(s) requested, {} used",
            self.file,
            profile,
            o.store.name(),
            o.seed,
            self.workers_requested,
            self.workers_used()
        );
        if let Some(symmetry) = self.symmetry_liveness()
            && symmetry.unreduced()
        {
            let _ = writeln!(
                out,
                "Declared SYMMETRY {}; whole-run policy unreduced (no symmetry reduction); actual progress follows.",
                symmetry.declared().unwrap_or_default()
            );
        }
        if let Phase::Checked(r) = &self.phase
            && let Some(why) = &r.workers.reason
        {
            let _ = writeln!(out, "{why}");
        }
        // A check that ends before the search (loading, ASSUME) names no
        // engine.
        if self.engine == crate::Engine::Auto {
            if let Some(f) = &self.fallback {
                let _ = writeln!(
                    out,
                    "engine auto: the reference engine ran; the core refused ({}): {}",
                    f.stage, f.reason
                );
            } else if self.core.is_some() {
                let _ = writeln!(out, "engine auto: the core ran");
            }
        }
        if let Some(request) = &self.checkpoint_request {
            let _ = writeln!(
                out,
                "native checkpoint {:?}, counters {:?}; trusted local archive, not an independent certificate",
                request.action, request.counters
            );
            if let Some(receipt) = &self.checkpoint_receipt {
                let _ = writeln!(
                    out,
                    "{}: {} discoveries, {} queued, next level{}",
                    receipt.status, receipt.discovered, receipt.queued, receipt.next_level
                );
            }
        }
        match &self.phase {
            Phase::BudgetStopped(stopped) => {
                let _ = writeln!(out, "{stopped}; no verdict produced");
            }
            Phase::SpecRejected(a) => out.push_str(&a.render_human()),
            Phase::ConfigRejected(c) => out.push_str(&c.render_human()),
            Phase::Unlowered(why) => {
                let _ = writeln!(out, "unsupported: {why}; no verdict produced");
            }
            Phase::Temporal(r) => out.push_str(&temporal_human(r)),
            Phase::TemporalError(e) => {
                let _ = writeln!(
                    out,
                    "{}: {e}; no verdict produced",
                    temporal_failure_kind(e)
                );
                let _ = writeln!(out, "Execution stage: {}", e.stage().name());
                if let Some(explanation) = crate::diagnostics::explain(e.error().code()) {
                    let _ = writeln!(out, "Help: {}", explanation.remediation);
                }
                if let Some(capture) = e.exploration() {
                    for (_, warning) in &capture.warnings {
                        let _ = writeln!(out, "Warning: {warning}");
                    }
                    for line in &capture.output {
                        let _ = writeln!(out, "{line}");
                    }
                    let stats = &capture.stats;
                    let completion = match e.capture_completed() {
                        Some(true) => "completed",
                        Some(false) => "incomplete",
                        None => "completion unknown",
                    };
                    let _ = writeln!(
                        out,
                        "Capture {completion}: {} generated, {} distinct, depth {}; no temporal verdict.",
                        stats.generated, stats.distinct, stats.depth
                    );
                } else if !matches!(e.error(), crate::temporal::Error::Binding(_)) {
                    for message in e.binding_output() {
                        match message {
                            dive_model::Message::Print(text) => {
                                let _ = writeln!(out, "{text}");
                            }
                            dive_model::Message::Warning { message, .. } => {
                                let _ = writeln!(out, "Warning: {message}");
                            }
                        }
                    }
                }
                if let Some((_, document, _)) = e.capture_row_error() {
                    let _ = writeln!(out, "The behavior up to this point is:");
                    for (index, state) in document.states.iter().enumerate() {
                        let _ = writeln!(out, "State {}: {}", index + 1, json!(state));
                    }
                }
            }
            Phase::Checked(r) => {
                for (_, w) in &r.warnings {
                    let _ = writeln!(out, "Warning: {w}");
                }
                for line in &r.output {
                    let _ = writeln!(out, "{line}");
                }
                out.push_str(&dive_check::render_human(r, o.eval.ctx));
            }
        }
        out
    }
}

/// The counterexample in `dive-trace`'s format, the
/// document the independent validator reads (ADR 0016): `dive
/// validate-trace` and the conformance harness take it from
/// `counterexample.trace`. `null` when a value has no JSON form or the
/// verdict has no ending the format names.
fn trace_doc(
    r: &dive_check::CheckResult,
    recorded: &[dive_check::TraceState],
    ctx: dive_value::Ctx,
    next_endpoint: Option<&dive_trace::NextStateEndpoint>,
    partial_endpoint: Option<&dive_trace::PartialNextEndpoint>,
    candidate_endpoint: Option<&dive_trace::CandidateStateEndpoint>,
    view_endpoint: Option<&dive_trace::ViewSourceEndpoint>,
) -> Result<Value, String> {
    use dive_check::Verdict as V;
    use dive_trace::Ending;
    let ending = match &r.verdict {
        V::InvariantViolated { property, .. } => Ending::Invariant(property.clone()),
        V::PropertyViolatedInitially { property } | V::ActionPropertyViolated { property } => {
            Ending::Property(property.clone())
        }
        V::Deadlock => Ending::Deadlock,
        V::NotCompletelySpecified { initial: false, .. } if partial_endpoint.is_some() => {
            Ending::PartialNext {
                endpoint: partial_endpoint
                    .cloned()
                    .ok_or("missing partial Next source")?,
            }
        }
        // A named word preserves its original cause. The independent reader
        // must separately prove the initial2147/2102 diagnostic relationship;
        // the successor2111 relationship retains its existing checks.
        V::EvaluationError {
            site: dive_check::ErrorSite::Invariant,
            code,
            reported,
            verified: true,
            invariant: Some(name),
            cause,
            ..
        } if (*reported == 2102 && *code == 2147)
            || (*reported == 2111
                && (matches!(*code, 2111 | 2103) || *code == cause.ec().code())) =>
        {
            Ending::InvariantError {
                name: name.clone(),
                ec: cause.ec().name().to_owned(),
            }
        }
        V::EvaluationError {
            site: dive_check::ErrorSite::Next,
            cause,
            ..
        } if next_endpoint.is_some() => Ending::NextStateError {
            ec: cause.ec().name().to_owned(),
            endpoint: next_endpoint.cloned().ok_or("missing Next error source")?,
        },
        V::EvaluationError { cause, .. } if view_endpoint.is_some() => Ending::ViewSourceError {
            ec: cause.ec().name().to_owned(),
            endpoint: view_endpoint.cloned().ok_or("missing VIEW source")?,
        },
        V::EvaluationError { cause, .. } if candidate_endpoint.is_some() => {
            Ending::CandidateStateError {
                ec: cause.ec().name().to_owned(),
                endpoint: candidate_endpoint
                    .cloned()
                    .ok_or("missing candidate source")?,
            }
        }
        V::EvaluationError { cause, .. } => {
            if cause.ec() == dive_eval::Ec::AssertFailed {
                Ending::Assertion
            } else {
                Ending::Error {
                    ec: Some(cause.ec().name().to_owned()),
                }
            }
        }
        _ => return Err("verdict has no supported trace ending".into()),
    };
    let mut states = Vec::with_capacity(recorded.len());
    let mut labels = Vec::with_capacity(recorded.len());
    for (i, s) in recorded.iter().enumerate() {
        if s.partial.is_some() {
            if i + 1 != recorded.len() || !matches!(ending, Ending::PartialNext { .. }) {
                return Err("partial bindings must be a separate final terminal descriptor".into());
            }
            continue;
        }
        let mut m = serde_json::Map::new();
        for (k, name) in r.var_names.iter().enumerate() {
            let Some(v) = s.values.get(k) else {
                return Err("counterexample has an unassigned variable".into());
            };
            m.insert(name.clone(), dive_trace::value_json::to_json(v, ctx)?);
        }
        states.push(m);
        labels.push(if i == 0 {
            None
        } else {
            s.action.as_ref().map(|name| dive_trace::Label {
                name: name.clone(),
                location: Some(s.label.clone()),
            })
        });
    }
    let doc = dive_trace::TraceDoc {
        variables: r.var_names.clone(),
        states,
        labels,
        ending: Some(ending),
        length: Some(recorded.len()),
        source: dive_trace::Source::Dive,
        properties: Vec::new(),
    };
    Ok(doc.to_json())
}

/// The `stopped` block of a run its budget stopped (ADR 0107).
pub(crate) fn stopped_json(stopped: &dive_check::budget::Stopped) -> Value {
    json!({
        "resource": stopped.resource.name(),
        "limit": stopped.limit,
        "observed": stopped.observed,
        "elapsed_ms": stopped.elapsed_ms,
        "unit": match stopped.resource {
            dive_check::budget::Resource::Time => "ms",
            dive_check::budget::Resource::Memory => "bytes",
            dive_check::budget::Resource::States => "states",
            dive_check::budget::Resource::Depth => "levels",
        },
        "phase": stopped.phase.name(),
        "hint": stopped.hint(),
    })
}

fn temporal_planning_evaluation(error: &crate::temporal::Error) -> Option<&dive_eval::EvalError> {
    match error {
        crate::temporal::Error::Analysis(dive_liveness::analyze::AnalysisError::Planning(
            dive_liveness::plan::PlanningError::Evaluation { error, .. },
        )) => Some(error),
        _ => None,
    }
}

fn temporal_action_evaluation(failure: &crate::temporal::Failure) -> Option<&dive_eval::EvalError> {
    let action = failure.action_evaluation()?;
    // TLC combines negated claims before simplification; this backend still
    // analyzes them separately. Do not invent a verified error for an action
    // that a different property could eliminate from the combined formula.
    let cause = action.core_cause()?;
    (action.properties() == 1
        && action.direct_operand()
        && cause.class() == dive_eval::ErrorClass::Eval
        && cause.ec() == dive_eval::Ec::General
        && !cause.is_eval_exception())
    .then_some(cause)
}

fn temporal_evaluation(failure: &crate::temporal::Failure) -> Option<&dive_eval::EvalError> {
    failure
        .capture_row_error()
        .map(|(cause, _, _)| cause)
        .or_else(|| temporal_action_evaluation(failure))
        .or_else(|| temporal_planning_evaluation(failure.error()))
}

fn temporal_startup_tautology(failure: &crate::temporal::Failure) -> bool {
    matches!(
        failure.error(),
        crate::temporal::Error::Analysis(
            dive_liveness::analyze::AnalysisError::StartupTautology { .. }
        )
    )
}

fn temporal_failure_kind(failure: &crate::temporal::Failure) -> &'static str {
    use crate::temporal::Error;
    use dive_liveness::analyze::AnalysisError;
    use dive_liveness::formula::FormulaError;
    use dive_liveness::plan::PlanningError;
    use dive_liveness::product::ProductError;
    if matches!(failure.error(), Error::Analysis(AnalysisError::Stopped(_))) {
        return "stopped";
    }
    if temporal_startup_tautology(failure) || failure.capture_row_error().is_some() {
        return "evaluation-error";
    }
    if let Some(action) = failure.action_evaluation() {
        return match action.cause().class() {
            dive_eval::ErrorClass::Eval if temporal_action_evaluation(failure).is_some() => {
                "evaluation-error"
            }
            dive_eval::ErrorClass::Eval | dive_eval::ErrorClass::Unsupported => "unsupported",
            dive_eval::ErrorClass::ResourceLimit => "resource-limit",
            dive_eval::ErrorClass::Bug | dive_eval::ErrorClass::Interrupted => "internal-error",
        };
    }
    match failure.error() {
        Error::Analysis(AnalysisError::Exploration(result))
            if matches!(result.verdict, Verdict::BudgetMeasurement { .. }) =>
        {
            "internal-error"
        }
        Error::Analysis(AnalysisError::Planning(PlanningError::Evaluation { .. })) => {
            "evaluation-error"
        }
        Error::NativeValidation { report, .. }
        | Error::Validation { report, .. }
        | Error::PrefixValidation { report, .. }
        | Error::EndpointValidation { report }
            if matches!(report.validation, dive_trace::Validation::Invalid(_)) =>
        {
            "internal-error"
        }
        Error::Thread(_)
        | Error::Document(_)
        | Error::Analysis(
            AnalysisError::Inconsistent(_)
            | AnalysisError::CaptureThread(_)
            | AnalysisError::BudgetStartup(_)
            | AnalysisError::BudgetMeasurement(_)
            | AnalysisError::Tableau(FormulaError::Reference(_))
            | AnalysisError::Planning(PlanningError::Formula {
                error: FormulaError::Reference(_),
                ..
            }),
        ) => "internal-error",
        Error::Binding(f) => match f.error.kind {
            dive_model::ErrorKind::Bug => "internal-error",
            dive_model::ErrorKind::Tlc(_) | dive_model::ErrorKind::Eval { .. } => "model-error",
            dive_model::ErrorKind::Unsupported { .. } => "unsupported",
        },
        Error::ValidationResource(_)
        | Error::Analysis(
            AnalysisError::Product(ProductError::Limit(_))
            | AnalysisError::Limit(_)
            | AnalysisError::Tableau(FormulaError::Limit(_))
            | AnalysisError::Planning(PlanningError::Formula {
                error: FormulaError::Limit(_),
                ..
            }),
        ) => "resource-limit",
        _ => "unsupported",
    }
}

fn temporal_complete(report: &crate::temporal::Report) -> Option<&dive_check::capture::Complete> {
    match report.outcome() {
        crate::temporal::Outcome::Holds(c) => Some(c),
        crate::temporal::Outcome::Violation(v) => Some(&v.candidate().exploration),
        crate::temporal::Outcome::Safety(_)
        | crate::temporal::Outcome::PrefixViolation(_)
        | crate::temporal::Outcome::NativeHolds(_)
        | crate::temporal::Outcome::NativeViolation(_)
        | crate::temporal::Outcome::NativePrefixViolation(_) => None,
    }
}

fn temporal_native_complete(
    report: &crate::temporal::Report,
) -> Option<&dive_check::capture::CompleteEvidence> {
    match report.outcome() {
        crate::temporal::Outcome::NativeHolds(evidence) => Some(evidence),
        crate::temporal::Outcome::NativeViolation(word) => Some(word.evidence()),
        _ => None,
    }
}

fn producer_facts(
    result: &CheckResult,
    stats: dive_check::Stats,
) -> dive_liveness::analyze::ExplorationEvidence {
    dive_liveness::analyze::ExplorationEvidence {
        stats,
        workers: result.workers.clone(),
        collision: result.collision.clone(),
        output: result.output.clone(),
        warnings: result.warnings.clone(),
    }
}

fn temporal_evidence(
    report: &crate::temporal::Report,
) -> Option<dive_liveness::analyze::ExplorationEvidence> {
    if let Some(complete) = temporal_complete(report) {
        return Some(complete.into());
    }
    if let Some(complete) = temporal_native_complete(report) {
        return Some(producer_facts(complete.safety_result(), complete.stats()));
    }
    if let crate::temporal::Outcome::PrefixViolation(word) = report.outcome() {
        return Some((&word.candidate().exploration).into());
    }
    if let crate::temporal::Outcome::NativePrefixViolation(word) = report.outcome() {
        return Some(producer_facts(
            word.evidence().safety_result(),
            word.evidence().stats(),
        ));
    }
    None
}

fn temporal_word(report: &crate::temporal::Report) -> Option<(&str, &dive_trace::TraceDoc, usize)> {
    match report.outcome() {
        crate::temporal::Outcome::Violation(word) => Some((
            &word.candidate().target_property,
            word.document(),
            word.candidate().back_to,
        )),
        crate::temporal::Outcome::PrefixViolation(word) => Some((
            &word.candidate().target_property,
            word.document(),
            word.candidate().back_to,
        )),
        crate::temporal::Outcome::NativeViolation(word) => Some((
            &word.word().target_property,
            word.document(),
            word.word().back_to,
        )),
        crate::temporal::Outcome::NativePrefixViolation(word) => Some((
            &word.word().target_property,
            word.document(),
            word.word().back_to,
        )),
        crate::temporal::Outcome::Holds(_)
        | crate::temporal::Outcome::NativeHolds(_)
        | crate::temporal::Outcome::Safety(_) => None,
    }
}

fn periodic_json(checks: dive_liveness::analyze::periodic::Checks) -> Value {
    json!({"min_states": checks.schedule.min_states,
        "growth_factor": checks.schedule.growth_factor,
        "prefix_checks": checks.prefixes, "last_level": checks.last_level, "last_size": checks.last_size,
        "exhausted": checks.exhausted, "final_check": checks.final_check,
        "bounds_scope": "capture cumulative; label and product per property per check; no total-run work or byte cap"})
}

const TEMPORAL_NOTE: &str = "Bounded short-lasso analysis: TLC-exact lasso selection and periodic timing are not claimed. Only independently validated periodic counterexamples are returned; prefix statistics describe the actual committed boundary.";

fn temporal_evidence_json(
    doc: &mut Value,
    capture: &dive_liveness::analyze::ExplorationEvidence,
    tlc_fingerprints: bool,
) {
    let s = &capture.stats;
    doc["statistics"] = json!({
        "generated": s.generated, "distinct": s.distinct,
        "left_on_queue": s.left_on_queue, "depth": s.depth,
        "initial_generated": s.initial_generated, "reported_by_tlc": false,
    });
    doc["output"] = json!(capture.output);
    doc["warnings"] = json!(
        capture
            .warnings
            .iter()
            .map(|(code, message)| { json!({"code": code, "message": message}) })
            .collect::<Vec<_>>()
    );
    if let Some(collision) = &capture.collision {
        doc["collision"] = json!({
            "store": "fingerprint", "scheme": if tlc_fingerprints {"tlc"} else {"dive"},
            "optimistic": collision.optimistic, "actual": collision.actual,
            "assumption": "fingerprints behave as random 64-bit values (TLC's assumption too)",
        });
    }
}

fn temporal_json(doc: &mut Value, report: &crate::temporal::Report, tlc_fingerprints: bool) {
    let Some(evidence) = temporal_evidence(report) else {
        doc["verdict"] = json!("internal-error");
        doc["message"] = json!("safety stop was not routed through the safety report");
        return;
    };
    let complete =
        temporal_complete(report).is_some() || temporal_native_complete(report).is_some();
    doc["execution"] = json!({"stage": if complete {"complete"} else {"prefix"}, "capture_completed": complete, "exhausted": complete});
    if let crate::temporal::Outcome::PrefixViolation(word) = report.outcome() {
        doc["execution"]["level"] = json!(word.candidate().exploration.level());
    }
    if let crate::temporal::Outcome::NativePrefixViolation(word) = report.outcome() {
        doc["execution"]["level"] = json!(word.evidence().level());
    }
    if let Some(checks) = report.periodic() {
        doc["periodic"] = periodic_json(checks);
    }
    temporal_evidence_json(doc, &evidence, tlc_fingerprints);
    doc["notes"] = json!([TEMPORAL_NOTE]);
    doc["elapsed_ms"] = json!(u64::try_from(report.elapsed().as_millis()).unwrap_or(u64::MAX));
    if let Some((_, document, back_to)) = temporal_word(report) {
        doc["property"] = json!(temporal_property_names(&document.properties));
        doc["properties"] = json!(document.properties);
        doc["counterexample"] = json!({
            "length": document.states.len(), "ending": "lasso", "back_to": back_to + 1,
            "states": document.states.iter().enumerate().map(|(i,s)| json!({"index":i+1,"label":null,"action":null,"values":s})).collect::<Vec<_>>(),
            "trace": document.to_json(), "validation": "valid",
        });
    }
}

// Pinned TLC MP::TLC_TEMPORAL_PROPERTY_VIOLATED names the properties false
// on the actual printed word, in configured order (ADR 0126).
fn temporal_property_names(names: &[String]) -> String {
    match names.split_last() {
        None => String::new(),
        Some((last, [])) => last.clone(),
        Some((last, [first])) => format!("{first} and {last}"),
        Some((last, rest)) => format!("{}, and {last}", rest.join(", ")),
    }
}

fn temporal_human(report: &crate::temporal::Report) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let Some(complete) = temporal_evidence(report) else {
        return "internal-error: unrouted temporal safety stop; no verdict produced\n".to_owned();
    };
    for (_, warning) in &complete.warnings {
        let _ = writeln!(out, "Warning: {warning}");
    }
    for line in &complete.output {
        let _ = writeln!(out, "{line}");
    }
    if let Some(reason) = &complete.workers.reason {
        let _ = writeln!(out, "{reason}");
    }
    if matches!(
        report.outcome(),
        crate::temporal::Outcome::Holds(_) | crate::temporal::Outcome::NativeHolds(_)
    ) {
        let _ = writeln!(
            out,
            "Complete final temporal analysis: no error has been found."
        );
    } else if let Some((_, document, back_to)) = temporal_word(report) {
        let names = temporal_property_names(&document.properties);
        if document.properties.len() == 1 {
            let _ = writeln!(out, "Temporal property {names} was violated.");
        } else {
            let _ = writeln!(out, "Temporal properties {names} were violated.");
        }
        let _ = writeln!(out, "Independently validated counterexample:");
        for (i, state) in document.states.iter().enumerate() {
            let _ = writeln!(out, "State {}: {}", i + 1, json!(state));
        }
        let _ = writeln!(out, "Back to state {}.", back_to + 1);
    }
    if let Some(checks) = report.periodic() {
        let _ = writeln!(
            out,
            "Deterministic liveness: {} prefix checks, final check {}, exhausted {}.",
            checks.prefixes, checks.final_check, checks.exhausted
        );
    }
    let s = &complete.stats;
    let _ = writeln!(
        out,
        "{} states generated, {} distinct states, depth {}.",
        s.generated, s.distinct, s.depth
    );
    if let Some(c) = &complete.collision {
        let _ = writeln!(
            out,
            "Fingerprint collision probability (random 64-bit assumption): optimistic {}, actual {}.",
            c.optimistic,
            c.actual.as_deref().unwrap_or("not reported")
        );
    } else {
        let _ = writeln!(out, "Exact full-state storage.");
    }
    let _ = writeln!(out, "{TEMPORAL_NOTE}");
    out
}

fn temporal_limits_json(configuration: crate::temporal::Configuration) -> Value {
    use dive_liveness::analyze::GraphLabelPolicy;
    use dive_liveness::product::AcceptancePolicy;
    let limits = configuration.limits;
    let graph_labels = match configuration.graph_labels {
        GraphLabelPolicy::Cells(cells) => json!({"kind": "cells", "cells": cells}),
        GraphLabelPolicy::NoCellLimit => json!({"kind": "run-budgets"}),
    };
    let (acceptance, bytes, work) = match configuration.acceptance {
        AcceptancePolicy::Legacy => (
            json!({"kind": "cells", "cells": limits.product.acceptance_cells}),
            None,
            None,
        ),
        AcceptancePolicy::Buffers { bytes, work } => (
            json!({"kind": "buffers", "bytes": bytes, "materialization_work": work}),
            Some(bytes),
            Some(work),
        ),
    };
    json!({
        "kind": "entry-work-counts-and-acceptance-buffers", "states": limits.capture.states,
        "edges": limits.capture.edges, "generated": limits.capture.generated,
        "successors_per_unit": limits.capture.successors_per_unit,
        "max_expansion_units": dive_check::capture::MAX_EXPANSION_UNITS,
        "properties": limits.properties,
        "cell_limits": {
            "graph_labels": graph_labels,
            "attribution_rows": {"kind": "cells", "cells": limits.atom_cells},
            "word_truth": {"kind": "cells", "cells": limits.atom_cells},
        },
        "formula_nodes": limits.formula.formula_nodes,
        "tableau_nodes": limits.formula.tableau_nodes,
        "formula_steps": limits.formula.expansion_steps,
        "product_vertices": limits.product.vertices,
        "product_edges": limits.product.edges,
        "acceptance_buffer_bytes": bytes,
        "acceptance_materialization_work": work,
        "acceptance_policy": acceptance,
        "total_memory_budget": false,
        "product_steps": limits.product.steps,
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, reason = "test assertions")]
mod evidence_controls {
    use super::*;

    fn limited_report(stage: dive_liveness::analyze::Stage) -> CheckReport {
        let source = "---- MODULE M ----\nEXTENDS TLC\nASSUME PrintT(\"boot\")\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>x\n====\n";
        let analysis = dive_sema::analyze_source("M.tla", source, &Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("configuration");
        let mut opts = CheckOptions::tlc(0, 0);
        opts.workers = 4;
        let mut limits = dive_liveness::analyze::Limits::default();
        match stage {
            dive_liveness::analyze::Stage::Product => limits.product.acceptance_cells = 0,
            dive_liveness::analyze::Stage::Capture => limits.capture.states = 1,
            _ => limits.properties = 0,
        }
        let phase = match crate::temporal::check_final_with_evidence(&ir, &cfg, &opts, limits) {
            Err(failure) => Phase::TemporalError(Box::new(failure)),
            Ok(report) => match report.outcome() {
                crate::temporal::Outcome::Safety(stop) => Phase::Checked(stop.clone()),
                _ => panic!("a resource bound cannot produce a temporal verdict"),
            },
        };
        CheckReport {
            checkpoint_request: None,
            checkpoint_receipt: None,
            file: "M.tla".into(),
            config: "M.cfg".into(),
            profile: Profile::Tlc,
            opts,
            seed_given: true,
            symmetry_liveness_explicit: false,
            fp_given: true,
            workers_requested: 4,
            temporal: TemporalMode::FinalShort,
            budget_sources: ("none", "none"),
            memory_requested_source: "none",
            memory_samples: None,
            temporal_configuration: crate::temporal::Configuration {
                limits,
                acceptance: dive_liveness::product::AcceptancePolicy::Legacy,
                graph_labels: dive_liveness::analyze::GraphLabelPolicy::Cells(limits.atom_cells),
            },
            phase,
            core: None,
            next_error_endpoint: None,
            candidate_error_endpoint: None,
            view_error_endpoint: None,
            partial_terminal_endpoint: None,
            engine: crate::Engine::Reference,
            fallback: None,
        }
    }

    #[test]
    fn owner_bounds_serialize_actual_custom_configuration_even_before_capture() {
        use dive_liveness::{
            analyze::{GraphLabelPolicy, Limits},
            product::AcceptancePolicy,
        };
        let mut report = limited_report(dive_liveness::analyze::Stage::Product);
        let source = "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>x\n====\n";
        let analysis = dive_sema::analyze_source("M.tla", source, &Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("config");
        let limits = Limits {
            properties: 0,
            atom_cells: 31,
            capture: dive_check::capture::Limits {
                states: 17,
                ..dive_check::capture::Limits::default()
            },
            ..Limits::default()
        };
        let failure = crate::temporal::check_final_with_policies(
            &ir,
            &cfg,
            &report.opts,
            limits,
            AcceptancePolicy::Buffers {
                bytes: 73,
                work: 79,
            },
            GraphLabelPolicy::Cells(19),
        )
        .expect_err("configured planning refusal");
        assert_eq!(failure.capture_completed(), Some(false));
        report.phase = Phase::TemporalError(Box::new(failure));
        let document = report.to_json();
        assert_eq!(document["schema_version"], 3);
        let bounds = &document["options"]["temporal_bounds"];
        assert!(bounds.get("atom_cells").is_none());
        assert_eq!(bounds["states"], 17);
        assert_eq!(bounds["properties"], 0);
        assert_eq!(
            bounds["cell_limits"],
            json!({"graph_labels":{"kind":"cells","cells":19},"attribution_rows":{"kind":"cells","cells":31},"word_truth":{"kind":"cells","cells":31}})
        );
        assert_eq!(bounds["acceptance_buffer_bytes"], 73);
        assert_eq!(bounds["acceptance_materialization_work"], 79);
        assert_eq!(
            bounds["acceptance_policy"],
            json!({"kind":"buffers","bytes":73,"materialization_work":79})
        );
        assert_eq!(document["execution"]["capture_completed"], false);
    }

    #[test]
    fn completed_reports_serialize_actual_zero_and_custom_owner_policies() {
        use dive_liveness::{
            analyze::{GraphLabelPolicy, Limits},
            product::AcceptancePolicy,
        };
        let source = "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>(x \\in {FALSE,TRUE})\n====\n";
        let analysis = dive_sema::analyze_source("M.tla", source, &Options::default());
        assert!(analysis.accepted());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("config");
        for graph_labels in [GraphLabelPolicy::Cells(6), GraphLabelPolicy::NoCellLimit] {
            for acceptance in [
                AcceptancePolicy::Legacy,
                AcceptancePolicy::Buffers {
                    bytes: 1_000_003,
                    work: 1_000_019,
                },
            ] {
                let mut report = limited_report(dive_liveness::analyze::Stage::Product);
                let mut limits = Limits {
                    atom_cells: 0,
                    ..Limits::default()
                };
                limits.product.acceptance_cells = 64;
                let actual = crate::temporal::check_final_with_policies(
                    &ir,
                    &cfg,
                    &report.opts,
                    limits,
                    acceptance,
                    graph_labels,
                )
                .expect("holding result within custom policies");
                assert!(matches!(
                    actual.outcome(),
                    crate::temporal::Outcome::Holds(_)
                ));
                report.phase = Phase::Temporal(Box::new(actual));
                let document = report.to_json();
                assert_eq!(document["schema_version"], 3);
                assert_eq!(document["verdict"], "success");
                assert_eq!(document["execution"]["capture_completed"], true);
                assert_eq!(document["statistics"]["distinct"], 2);
                let bounds = &document["options"]["temporal_bounds"];
                let graph = match graph_labels {
                    GraphLabelPolicy::Cells(cells) => json!({"kind":"cells","cells":cells}),
                    GraphLabelPolicy::NoCellLimit => json!({"kind":"run-budgets"}),
                };
                assert_eq!(bounds["cell_limits"]["graph_labels"], graph);
                for owner in ["attribution_rows", "word_truth"] {
                    assert_eq!(
                        bounds["cell_limits"][owner],
                        json!({"kind":"cells","cells":0})
                    );
                }
                match acceptance {
                    AcceptancePolicy::Legacy => {
                        assert_eq!(
                            bounds["acceptance_policy"],
                            json!({"kind":"cells","cells":64})
                        );
                        assert_eq!(bounds.get("acceptance_buffer_bytes"), Some(&Value::Null));
                        assert_eq!(
                            bounds.get("acceptance_materialization_work"),
                            Some(&Value::Null)
                        );
                    }
                    AcceptancePolicy::Buffers { bytes, work } => {
                        assert_eq!(
                            bounds["acceptance_policy"],
                            json!({"kind":"buffers","bytes":bytes,"materialization_work":work})
                        );
                        assert_eq!(bounds["acceptance_buffer_bytes"], bytes);
                        assert_eq!(bounds["acceptance_materialization_work"], work);
                    }
                }
            }
        }
    }

    #[test]
    fn terminal_sampling_preserves_completed_progress_without_a_proof() {
        let source = "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>(x \\/ ~x)\n====\n";
        let analysis = dive_sema::analyze_source("M.tla", source, &Options::default());
        assert!(analysis.accepted());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("cfg");
        for finished in [
            Err(dive_check::budget::MeasurementError::unavailable(
                dive_check::budget::SamplePoint::Final,
            )),
            Ok(Some(dive_check::budget::Exhausted {
                resource: dive_check::budget::Resource::Memory,
                limit: 100,
                observed: 101,
                elapsed_ms: 2,
            })),
        ] {
            let temporal = crate::temporal::check_final(
                &ir,
                &cfg,
                &CheckOptions::tlc(0, 0),
                dive_liveness::analyze::Limits::default(),
            )
            .expect("actual holding result");
            assert!(matches!(
                temporal.outcome(),
                crate::temporal::Outcome::Holds(_)
            ));
            let mut report = limited_report(dive_liveness::analyze::Stage::Product);
            report.phase = Phase::Temporal(Box::new(temporal));
            report.finish_budget(finished, Duration::from_millis(2));
            let json = report.to_json();
            assert_eq!(json["execution"]["stage"], "finalization", "{json}");
            assert_eq!(json["execution"]["capture_completed"], true);
            assert_eq!(json["statistics"]["distinct"], 2);
            assert_eq!(json["statistics"]["generated"], 3);
            assert_eq!(json["statistics"]["depth"], 2);
            assert_eq!(json["statistics"]["exhaustive"], false);
            assert!(json["tlc"].is_null());
            assert!(json["counterexample"].is_null());
            if finished.is_err() {
                assert_eq!(json["verdict"], "internal-error");
                assert_eq!(
                    json["diagnostics"][0]["code"],
                    dive_check::budget::MEASUREMENT_FAILURE_CODE
                );
                assert!(json["stopped"].is_null());
            } else {
                assert_eq!(json["verdict"], "stopped");
                assert_eq!(json["stopped"]["phase"], "finalization");
                assert_eq!(json["stopped"]["observed"], 101);
            }
        }
    }

    #[test]
    fn a_safety_measurement_error_has_actionable_help_without_an_invented_span() {
        let source = "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\n====\n";
        let analysis = dive_sema::analyze_source("M.tla", source, &Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
            .into_config()
            .expect("cfg");
        let opts = CheckOptions::tlc(0, 0);
        let result = dive_check::check(&ir, &cfg, &opts);
        assert_eq!(result.verdict, Verdict::Success);
        let mut report = limited_report(dive_liveness::analyze::Stage::Product);
        report.temporal = TemporalMode::Ordinary;
        report.opts = opts;
        report.workers_requested = 1;
        report.phase = Phase::Checked(Box::new(result));
        report.finish_budget(
            Err(dive_check::budget::MeasurementError::unavailable(
                dive_check::budget::SamplePoint::Final,
            )),
            Duration::from_millis(2),
        );
        let json = report.to_json();
        assert_eq!(json["verdict"], "internal-error");
        assert_eq!(
            json["diagnostics"][0]["code"],
            dive_check::budget::MEASUREMENT_FAILURE_CODE
        );
        assert_eq!(json["diagnostics"][0]["cause"], json["message"]);
        assert!(
            json["diagnostics"][0]["help"]
                .as_str()
                .is_some_and(|help| help.contains("--max-memory=none"))
        );
        assert!(json["diagnostics"][0]["span"].is_null());
        assert_eq!(json["statistics"]["exhaustive"], false);
        assert_eq!(report.exit_code(), exit::INTERNAL);
        assert!(json["tlc"].is_null());
        assert!(json["stopped"].is_null());
        assert!(json["counterexample"].is_null());
    }

    #[test]
    fn an_already_exhausted_temporal_request_reports_startup_stop() {
        let source = "---- MODULE M ----\nVARIABLE x\nInit == x = FALSE\nNext == x' = ~x\nP == []<>x\n====\n";
        let analysis = dive_sema::analyze_source("M.tla", source, &Options::default());
        let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
        let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\n")
            .into_config()
            .expect("configuration");
        let watchdog =
            dive_check::budget::Budget::new(Some(std::time::Duration::from_millis(1)), None)
                .start()
                .expect("watchdog thread starts")
                .expect("watchdog");
        let started = std::time::Instant::now();
        while watchdog.budget().poll().expect("measurement").is_none()
            && started.elapsed() < std::time::Duration::from_secs(10)
        {
            std::thread::sleep(std::time::Duration::from_millis(1));
        }
        let opts = CheckOptions {
            budget: watchdog.budget().clone(),
            ..CheckOptions::tlc(0, 0)
        };
        let limits = dive_liveness::analyze::Limits::default();
        let Err(failure) = crate::temporal::check_final_with_evidence(&ir, &cfg, &opts, limits)
        else {
            panic!("an exhausted budget cannot produce a temporal result");
        };
        let mut report = limited_report(dive_liveness::analyze::Stage::Product);
        report.phase = Phase::TemporalError(Box::new(failure));
        let json = report.to_json();
        assert_eq!(json["schema_version"], TEMPORAL_CHECK_SCHEMA_VERSION);
        assert_eq!(json["verdict"], "stopped", "{json}");
        assert_eq!(json["stopped"]["resource"], "time");
        assert_eq!(json["stopped"]["phase"], "check-startup");
        assert_eq!(json["execution"]["stage"], "check-startup");
        assert_eq!(json["execution"]["capture_completed"], false);
        assert!(json["statistics"].is_null());
        assert!(json["tlc"].is_null());
        assert_eq!(report.exit_code_for(ExitConvention::Tlc), exit::STOPPED);
        assert_eq!(report.exit_code_for(ExitConvention::Native), exit::STOPPED);
    }

    #[test]
    fn failed_product_report_retains_capture_but_never_a_verdict() {
        let report = limited_report(dive_liveness::analyze::Stage::Product);
        let json = report.to_json();
        assert_eq!(json["schema_version"], TEMPORAL_CHECK_SCHEMA_VERSION);
        assert_eq!(json["verdict"], "resource-limit");
        assert_eq!(json["options"]["profile"], "tlc+overrides");
        assert_eq!(json["options"]["workers"]["used"], 4);
        assert_eq!(json["options"]["workers"]["requested"], 4);
        assert_eq!(
            json["options"]["temporal_bounds"]["successors_per_unit"],
            65_536
        );
        assert_eq!(json["options"]["temporal_bounds"]["max_expansion_units"], 8);
        assert_eq!(json["execution"]["stage"], "product");
        assert_eq!(json["execution"]["capture_completed"], true);
        assert_eq!(json["statistics"]["generated"], 3);
        assert_eq!(json["statistics"]["distinct"], 2);
        assert_eq!(json["statistics"]["depth"], 2);
        assert_eq!(json["statistics"]["reported_by_tlc"], false);
        assert_eq!(json["output"], json!(["\"boot\""]));
        assert!(json["counterexample"].is_null());
        assert!(json["tlc"].is_null());
        assert_eq!(report.exit_code(), exit::UNSUPPORTED);
        let human = report.render_human();
        assert!(human.contains("4 worker(s) requested, 4 used"));
        assert!(
            human.contains(
                "Capture completed: 3 generated, 2 distinct, depth 2; no temporal verdict."
            )
        );
        assert!(!human.contains("no search ran"));
    }

    #[test]
    fn planning_report_proves_no_capture_without_statistics() {
        let report = limited_report(dive_liveness::analyze::Stage::Planning);
        let json = report.to_json();
        assert_eq!(json["options"]["workers"]["used"], 0);
        assert_eq!(json["options"]["workers"]["reason"], "no search ran");
        assert_eq!(json["execution"]["stage"], "planning");
        assert_eq!(json["execution"]["capture_completed"], false);
        assert!(json["statistics"].is_null());
        assert!(json["collision"].is_null());
        assert_eq!(json["verdict"], "resource-limit");
    }
    #[test]
    fn capture_stop_report_retains_partial_work_and_stage() {
        let report = limited_report(dive_liveness::analyze::Stage::Capture);
        let json = report.to_json();
        assert_eq!(json["execution"]["stage"], "capture");
        assert_eq!(json["execution"]["capture_completed"], false);
        assert!(!report.render_human().contains("Capture completed:"));
        assert_eq!(json["options"]["workers"]["used"], 4);
        assert_eq!(json["verdict"], "resource-limit");
        assert!(!json["statistics"].is_null());
        // A resource stop retains its finite diagnostic prefix, not a
        // certified counterexample claim.
        assert_eq!(json["counterexample"]["ending"], "finite");
        assert!(json["counterexample"]["trace"].is_null());
        assert!(json["tlc"].is_null());
        assert_eq!(report.exit_code(), exit::UNSUPPORTED);
    }
    #[test]
    fn default_ceiling_resolution_failure_is_distinct_and_never_unlimited() {
        use dive_check::budget::{MeasurementKind, SamplePoint};
        let error = resolve_memory(BudgetSetting::Default, || None).expect_err("RAM unknown");
        assert_eq!(error.kind, MeasurementKind::DefaultCeiling);
        assert_eq!(error.point, SamplePoint::Initial);
        assert!(resolve_memory(BudgetSetting::Default, || Some(0)).is_err());
        assert_eq!(
            resolve_memory(BudgetSetting::Explicit(1), || None),
            Ok((Some(1), "explicit"))
        );
        assert_eq!(
            resolve_memory(BudgetSetting::Unlimited, || None),
            Ok((None, "none"))
        );
    }
}

#[cfg(test)]
#[path = "binding_report_controls.rs"]
mod binding_report_controls;

// Explanatory bound metadata only. It never supplies execution or proof facts.
fn symmetry_liveness_json(resolution: &dive_model::SymmetryLivenessResolution) -> Value {
    json!({
        "declared": resolution.declared(),
        "requested": resolution.requested().as_str(),
        "genuine_temporal": resolution.genuine_temporal(),
        "effective": resolution.effective(),
        "scope": if resolution.unreduced() { Some("whole-run") } else { None },
    })
}

#[cfg(test)]
#[allow(
    clippy::expect_used,
    reason = "finite configuration serialization assertions"
)]
mod product_vertex_report_controls {
    use super::*;

    #[test]
    fn actual_scalar_is_reported_without_changing_other_owner_settings() {
        let file = crate::temporal::file_configuration();
        assert_eq!(file.limits.product.vertices, usize::MAX);
        let mut legacy = file;
        legacy.limits = dive_liveness::analyze::Limits::default();
        assert_eq!(legacy.limits.product.vertices, 1_000_000);
        for vertices in [0, 1, 2, 1_000_000, usize::MAX] {
            let mut configured = legacy;
            configured.limits.product.vertices = vertices;
            let actual = temporal_limits_json(configured);
            assert_eq!(actual["product_vertices"], json!(vertices));
            let mut expected = temporal_limits_json(file);
            expected["product_vertices"] = json!(vertices);
            assert_eq!(actual, expected, "only the vertex scalar differs");
        }
    }
}
