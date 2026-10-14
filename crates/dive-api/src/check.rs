//! Model checking (`dive check`): load, bind and check a specification, and
//! report the result as a versioned JSON document or TLC-style text.

use std::path::PathBuf;
use std::sync::Arc;

use dive_check::{CheckOptions, CheckResult, Verdict};
use dive_eval::Profile;
use dive_sema::{Options, SearchPath};
use dive_store::StoreMode;
use serde_json::{Value, json};

use crate::{AnalyzeReport, ConfigReport};

/// Version of the JSON document produced by [`CheckReport::to_json`].
pub const CHECK_SCHEMA_VERSION: u32 = 1;

/// Version of the opt-in final-only temporal JSON document. Ordinary checks
/// retain schema 1; version 2 adds periodic endings, claims and mode metadata.
pub const TEMPORAL_CHECK_SCHEMA_VERSION: u32 = 2;

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

/// Temporal checking selected explicitly by a caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum TemporalMode {
    /// Ordinary checking, retaining current strict temporal refusals.
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
    /// `--action-composition off|tlc|tla` (D-022); `None` is the profile's
    /// setting: `tla` by default, `off` under `--compat=tlc` (TLC without
    /// `-Dtlc2.tool.impl.Tool.cdot=true`).
    pub action_composition: Option<dive_eval::ActionComposition>,
    /// The store.
    pub store: Store,
    /// `--seed`; a fresh seed otherwise (always reported).
    pub seed: Option<i64>,
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
    /// Explicit bounded final-only temporal checking. `FinalShort` changes TLC's
    /// witness selection and labels TLC profiles `tlc+overrides`.
    /// `Ordinary` retains unimplemented temporal scheduling refusals.
    pub temporal: TemporalMode,
}

/// The phase a check ended in.
#[derive(Debug)]
enum Phase {
    /// The specification was rejected by the front end.
    SpecRejected(Box<AnalyzeReport>),
    /// The configuration was rejected.
    ConfigRejected(Box<ConfigReport>),
    /// The specification could not be lowered (a refusal).
    Unlowered(String),
    /// The checker ran.
    Checked(Box<CheckResult>),
    /// Independently validated bounded final temporal analysis.
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
    fp_given: bool,
    workers_requested: usize,
    temporal: TemporalMode,
    phase: Phase,
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

/// Loads, binds and checks the request.
///
/// # Errors
/// [`crate::ApiError`] if the root file or the configuration cannot be read.
pub fn check_file(req: &CheckRequest) -> Result<CheckReport, crate::ApiError> {
    check_file_in(req, None)
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
            feature: "disk seen-state retention with native checkpoint/recovery is not implemented"
                .into(),
        });
    }
    if req.temporal != TemporalMode::Ordinary {
        return Err(crate::ApiError::Unsupported {
            feature: "native checkpoint with temporal checking is not implemented".into(),
        });
    }
    check_file_in(req, Some(checkpoint))
}

#[allow(
    clippy::too_many_lines,
    reason = "single ordered loader dispatch for ordinary and native recovery admission"
)]
fn check_file_in(
    req: &CheckRequest,
    checkpoint: Option<&dive_check::checkpoint::Request>,
) -> Result<CheckReport, crate::ApiError> {
    admit_storage_request(req)?;
    let profile = if req.compat_tlc {
        Profile::Tlc
    } else {
        Profile::Dive
    };
    let recovered_defaults = if let Some(request) = checkpoint
        && request.action == dive_check::checkpoint::Action::Recover
    {
        Some(
            dive_check::checkpoint::recovery_seed_fp(&request.path).map_err(|message| {
                crate::ApiError::Rejected {
                    message: format!("DIVE-S0400: {message}"),
                }
            })?,
        )
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
    opts.predecessor_directory
        .clone_from(&req.predecessor_directory);
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
        fp_given: req.fp.is_some(),
        workers_requested,
        temporal: req.temporal,
        phase: Phase::Unlowered(String::new()),
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
    let cfg = crate::parse_config_file(&cfg_path)?;
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
    report.phase = if let Some(request) = checkpoint {
        let mut request = request.clone();
        request.config_source_sha256 = Some(dive_check::checkpoint::config_source_hash(
            cfg.source_bytes(),
        ));
        let native = dive_check::checkpoint::check(&ir, &model, &report.opts, &request);
        report.checkpoint_receipt = native.receipt;
        Phase::Checked(Box::new(native.check))
    } else {
        check_backend(&ir, &model, &report.opts, req.temporal)
    };
    Ok(report)
}

fn check_backend(
    ir: &Arc<dive_ir::Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
    temporal: TemporalMode,
) -> Phase {
    if temporal == TemporalMode::FinalShort {
        return match crate::temporal::check_final_with_evidence(
            ir,
            config,
            options,
            dive_liveness::analyze::Limits::default(),
        ) {
            Ok(r) => match r.outcome() {
                crate::temporal::Outcome::Safety(s) => Phase::Checked(s.clone()),
                _ => Phase::Temporal(Box::new(r)),
            },
            Err(e) => Phase::TemporalError(Box::new(e)),
        };
    }
    Phase::Checked(Box::new(dive_check::check(ir, config, options)))
}

impl CheckReport {
    /// The safety checker's result, including a safety stop in final/short
    /// checking. Temporal holds/violations use [`Self::temporal_result`].
    #[must_use]
    pub fn result(&self) -> Option<&CheckResult> {
        match &self.phase {
            Phase::Checked(r) => Some(r),
            _ => None,
        }
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
            Phase::SpecRejected(_) | Phase::ConfigRejected(_) => "rejected",
            Phase::Unlowered(_) => "unsupported",
            Phase::Checked(r) => r.verdict.kind(),
            Phase::Temporal(r) => match r.outcome() {
                crate::temporal::Outcome::Holds(_) => "success",
                crate::temporal::Outcome::Violation(_) => "liveness-violation",
                crate::temporal::Outcome::Safety(_) => "internal-error",
            },
            Phase::TemporalError(e) => temporal_failure_kind(e.error()),
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
                crate::temporal::Outcome::Holds(_) => exit::SUCCESS,
                crate::temporal::Outcome::Violation(_) => {
                    if tlc {
                        13
                    } else {
                        exit::VIOLATION
                    }
                }
                crate::temporal::Outcome::Safety(_) => exit::INTERNAL,
            },
            Phase::TemporalError(e) => match temporal_failure_kind(e.error()) {
                "internal-error" => exit::INTERNAL,
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
                        temporal_planning_evaluation(e.error()).map_or(exit::INTERNAL, |error| {
                            dive_check::tlc_exit_status(error.ec().code())
                        })
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
                    Verdict::InternalError { .. } => exit::INTERNAL,
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
                    }) =>
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
            "frontier_codec_nodes_per_value_limit": o.frontier_memory_bytes.map(|_| dive_value::codec::FRONTIER_MAX_NODES),
            "predecessor_memory_bytes": o.predecessor_memory_bytes,
            "predecessor_path_steps_limit": o.predecessor_memory_bytes.map(|_| dive_store::predecessor::MAX_PATH_STEPS),
        });
        if let Some(disk) = &o.seen_disk {
            options["seen_disk"] = json!({"entries_limit": disk.limits.entries, "key_bytes_limit": disk.limits.key_bytes, "probe_steps_limit": disk.limits.probe_steps, "summary_pairs_limit": disk.limits.summary_pairs, "directory": disk.directory, "execution": "inline-fallback", "resident_key_collection_bytes": 0, "total_memory_budget": false});
        }
        if self.temporal == TemporalMode::FinalShort {
            options["lncheck"] = json!("final");
            options["lasso"] = json!("short");
            options["temporal_bounds"] = temporal_limits_json();
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
                "max_state_bytes": dive_check::checkpoint::MAX_STATE_BYTES,
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
            .map(|r| &r.workers)
            .or_else(|| {
                self.temporal_result()
                    .and_then(temporal_complete)
                    .map(|c| &c.workers)
            })
            .cloned()
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
            Phase::Temporal(r) => temporal_complete(r)
                .map_or(0, |c| c.workers.used)
                .to_string(),
            Phase::TemporalError(failure) => failure
                .workers(self.workers_requested)
                .map_or_else(|| "unknown".into(), |w| w.used.to_string()),
            _ => "0".into(),
        }
    }

    /// The JSON document (schema [`CHECK_SCHEMA_VERSION`] for ordinary checks;
    /// [`TEMPORAL_CHECK_SCHEMA_VERSION`] for explicit final-only requests):
    ///
    /// ```text
    /// { "schema_version": 1, "command": "check", "file", "config",
    ///   "verdict": "success" | "invariant-violation" | "property-violation"
    ///            | "action-property-violation" | "deadlock" | "assumption-failure"
    ///            | "postcondition-failure" | "assertion-failure" | "overflow"
    ///            | "evaluation-error" | "incomplete-state" | "model-error"
    ///            | "rejected" | "unsupported" | "resource-limit" | "internal-error",
    ///   "property": string | null, "message": string | null,
    ///   "error_site": string | null,
    ///   "tlc": { "code", "name", "exit_status", "reported", "verified" } | null,
    ///   "statistics": { "generated", "distinct", "left_on_queue", "depth",
    ///                   "initial_generated", "reported_by_tlc" } | null,
    ///   "counterexample": { "length", "ending": "finite",
    ///                       "states": [ { "index", "label", "action",
    ///                                     "values": { var: TLC JSON }, "text" } ],
    ///                       "trace": dive-trace document | null } | null,
    ///   "collision": { "store", "scheme", "optimistic", "actual", "assumption" } | null,
    ///   "output": [ string ], "warnings": [ { "code", "message" } ],
    ///   "diagnostics": [ ... ], "notes": [ string ],
    ///   "options": { "profile", "int", "choose_order", "explore_order",
    ///                "comparisons", "warnings", "store", "fingerprint",
    ///                "seed", "seed_source", "deadlock_flag",
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
    /// (a stop while generating initial states). Explicit final-only schema 2
    /// also declares the schedule/selection, fixed entry/work bounds and actual
    /// failure stage. Periodic traces include all configured property names and
    /// a stuttering ending or one-based lasso target; independent replay precedes
    /// any private-certified temporal violation.
    #[must_use]
    #[allow(clippy::too_many_lines, reason = "one document")]
    pub fn to_json(&self) -> Value {
        let mut doc = json!({
            "schema_version": if self.temporal == TemporalMode::Ordinary { CHECK_SCHEMA_VERSION } else { TEMPORAL_CHECK_SCHEMA_VERSION },
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
            "elapsed_ms": 0,
        });
        match &self.phase {
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
            Phase::Temporal(r) => temporal_json(&mut doc, r, self.opts.tlc_fingerprints),
            Phase::TemporalError(e) => {
                doc["message"] = json!(e.to_string());
                doc["diagnostics"] = json!([{"code": e.error().code(), "message": e.to_string()}]);
                doc["execution"] =
                    json!({"stage": e.stage().name(), "capture_completed": e.capture_completed()});
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
                } else if let Some(error) = temporal_planning_evaluation(e.error()) {
                    let code = error.ec().code();
                    doc["tlc"] = json!({
                        "code": code,
                        "name": error.ec().name(),
                        "exit_status": dive_check::tlc_exit_status(code),
                        "verified": error.ec() == dive_eval::Ec::ExpectedValue,
                    });
                }
                if let Some(capture) = e.exploration() {
                    temporal_evidence_json(&mut doc, capture, self.opts.tlc_fingerprints);
                    doc["notes"] = json!([
                        "Capture completed, but temporal checking failed; these statistics are exploration evidence, not a temporal verdict."
                    ]);
                } else if let crate::temporal::Error::Binding(failure) = e.error() {
                    doc["warnings"] = json!(
                        failure
                            .output
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
                        failure
                            .output
                            .iter()
                            .filter_map(|m| match m {
                                dive_model::Message::Print(text) => Some(text.clone()),
                                dive_model::Message::Warning { .. } => None,
                            })
                            .collect::<Vec<_>>()
                    );
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
                    });
                }
                doc["frontier_storage"] = json!({
                    "mode": if r.frontier.memory_bytes_per_queue.is_some() { "encoded-spill" } else { "memory-reference" },
                    "scope": "encoded current and next frontier queues only; legacy value-frontier heap, candidates, seen sets, traces, graphs and native archive work are not measured or bounded",
                    "memory_bytes_per_queue": r.frontier.memory_bytes_per_queue,
                    "max_record_bytes": r.frontier.max_record_bytes,
                    "max_codec_nodes_per_value": r.frontier.memory_bytes_per_queue.map(|_| dive_value::codec::FRONTIER_MAX_NODES),
                    "peak_retained_bytes": r.frontier.peak_retained_bytes,
                    "retained_bytes": r.frontier.retained_bytes,
                    "committed_spill_bytes": r.frontier.spilled_bytes,
                    "write_accounting_complete": r.frontier.failures.is_empty(),
                    "failures": r.frontier.failures,
                    "search_event_before_storage_failure": r.frontier.deciding_event_before_failure,
                });
                if self.temporal != TemporalMode::Ordinary {
                    doc["execution"] = json!({"stage": "capture", "capture_completed": false});
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
                    Verdict::AssumptionFalse { message }
                    | Verdict::EvaluationError { message, .. }
                    | Verdict::ModelError { message, .. }
                    | Verdict::PostconditionFalse { message, .. }
                    | Verdict::NotCompletelySpecified { message, .. }
                    | Verdict::ResourceLimit { message }
                    | Verdict::InternalError { message } => Value::from(message.as_str()),
                    Verdict::Unsupported { feature } => Value::from(feature.as_str()),
                    _ => Value::Null,
                };
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
                        "reported_by_tlc": r.tlc_prints_stats,
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
                    let transported = trace_doc(r, cx, ctx);
                    doc["counterexample"] = json!({
                        "length": states.len(),
                        "ending": "finite",
                        "states": states,
                        "trace": transported.unwrap_or(Value::Null),
                    });
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
        if let Phase::Checked(r) = &self.phase
            && let Some(why) = &r.workers.reason
        {
            let _ = writeln!(out, "{why}");
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
                    temporal_failure_kind(e.error())
                );
                let _ = writeln!(out, "Execution stage: {}", e.stage().name());
                if let Some(capture) = e.exploration() {
                    for (_, warning) in &capture.warnings {
                        let _ = writeln!(out, "Warning: {warning}");
                    }
                    for line in &capture.output {
                        let _ = writeln!(out, "{line}");
                    }
                    let stats = &capture.stats;
                    let _ = writeln!(
                        out,
                        "Capture completed: {} generated, {} distinct, depth {}; no temporal verdict.",
                        stats.generated, stats.distinct, stats.depth
                    );
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
    cx: &dive_check::Counterexample,
    ctx: dive_value::Ctx,
) -> Result<Value, String> {
    use dive_check::Verdict as V;
    use dive_trace::Ending;
    let ending = match &r.verdict {
        V::InvariantViolated { property, .. } => Ending::Invariant(property.clone()),
        V::PropertyViolatedInitially { property } | V::ActionPropertyViolated { property } => {
            Ending::Property(property.clone())
        }
        V::Deadlock => Ending::Deadlock,
        // The ending names the error itself, not the class TLC's site and
        // replay give it.
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
    let mut states = Vec::with_capacity(cx.states.len());
    let mut labels = Vec::with_capacity(cx.states.len());
    for (i, s) in cx.states.iter().enumerate() {
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
        length: Some(cx.states.len()),
        source: dive_trace::Source::Dive,
        properties: Vec::new(),
    };
    Ok(doc.to_json())
}

fn temporal_planning_evaluation(error: &crate::temporal::Error) -> Option<&dive_eval::EvalError> {
    match error {
        crate::temporal::Error::Analysis(dive_liveness::analyze::AnalysisError::Planning(
            dive_liveness::plan::PlanningError::Evaluation { error, .. },
        )) => Some(error),
        _ => None,
    }
}

fn temporal_failure_kind(error: &crate::temporal::Error) -> &'static str {
    use crate::temporal::Error;
    use dive_liveness::analyze::AnalysisError;
    use dive_liveness::formula::FormulaError;
    use dive_liveness::plan::PlanningError;
    use dive_liveness::product::ProductError;
    match error {
        Error::Analysis(AnalysisError::Planning(PlanningError::Evaluation { .. })) => {
            "evaluation-error"
        }
        Error::Validation { report, .. }
            if matches!(report.validation, dive_trace::Validation::Invalid(_)) =>
        {
            "internal-error"
        }
        Error::Thread(_)
        | Error::Document(_)
        | Error::Analysis(
            AnalysisError::Inconsistent(_)
            | AnalysisError::CaptureThread(_)
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
        Error::Analysis(
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
        crate::temporal::Outcome::Safety(_) => None,
    }
}

const TEMPORAL_NOTE: &str = "Bounded final/short analysis: TLC-exact lasso selection, finite-prefix stopping, temporal error classes and periodic scheduling are not claimed. Only independently validated periodic counterexamples are returned.";

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
    let Some(complete) = temporal_complete(report) else {
        doc["verdict"] = json!("internal-error");
        doc["message"] = json!("safety stop was not routed through the safety report");
        return;
    };
    let s = &complete.stats;
    doc["execution"] = json!({"stage": "complete", "capture_completed": true});
    doc["statistics"] = json!({
        "generated": s.generated, "distinct": s.distinct,
        "left_on_queue": s.left_on_queue, "depth": s.depth,
        "initial_generated": s.initial_generated,
        "reported_by_tlc": false,
    });
    doc["output"] = json!(complete.output);
    doc["warnings"] = json!(
        complete
            .warnings
            .iter()
            .map(|(c, m)| json!({"code":c,"message":m}))
            .collect::<Vec<_>>()
    );
    doc["notes"] = json!([TEMPORAL_NOTE]);
    doc["elapsed_ms"] = json!(u64::try_from(report.elapsed().as_millis()).unwrap_or(u64::MAX));
    if let Some(c) = &complete.collision {
        doc["collision"] = json!({
            "store": "fingerprint", "scheme": if tlc_fingerprints {"tlc"} else {"dive"},
            "optimistic": c.optimistic, "actual": c.actual,
            "assumption": "fingerprints behave as random 64-bit values (TLC's assumption too)",
        });
    }
    if let crate::temporal::Outcome::Violation(v) = report.outcome() {
        doc["property"] = json!(v.candidate().target_property);
        doc["properties"] = json!(v.document().properties);
        doc["counterexample"] = json!({
            "length": v.document().states.len(), "ending": "lasso",
            "back_to": v.candidate().back_to + 1,
            "states": v.document().states.iter().enumerate().map(|(i,s)| json!({"index":i+1,"label":null,"action":null,"values":s})).collect::<Vec<_>>(),
            "trace": v.document().to_json(),
            "validation": "valid",
        });
    }
}

fn temporal_human(report: &crate::temporal::Report) -> String {
    use std::fmt::Write as _;
    let mut out = String::new();
    let Some(complete) = temporal_complete(report) else {
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
    match report.outcome() {
        crate::temporal::Outcome::Holds(_) => {
            let _ = writeln!(
                out,
                "Complete final temporal analysis: no error has been found."
            );
        }
        crate::temporal::Outcome::Violation(v) => {
            let _ = writeln!(
                out,
                "Temporal properties {} were violated.",
                v.document().properties.join(", ")
            );
            let _ = writeln!(out, "Independently validated counterexample:");
            for (i, state) in v.document().states.iter().enumerate() {
                let _ = writeln!(out, "State {}: {}", i + 1, json!(state));
            }
            let _ = writeln!(out, "Back to state {}.", v.candidate().back_to + 1);
        }
        crate::temporal::Outcome::Safety(_) => {}
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

fn temporal_limits_json() -> Value {
    let limits = dive_liveness::analyze::Limits::default();
    json!({
        "kind": "entry-counts", "states": limits.capture.states,
        "edges": limits.capture.edges, "generated": limits.capture.generated,
        "successors_per_unit": limits.capture.successors_per_unit,
        "max_expansion_units": dive_check::capture::MAX_EXPANSION_UNITS,
        "properties": limits.properties, "atom_cells": limits.atom_cells,
        "formula_nodes": limits.formula.formula_nodes,
        "tableau_nodes": limits.formula.tableau_nodes,
        "formula_steps": limits.formula.expansion_steps,
        "product_vertices": limits.product.vertices,
        "product_edges": limits.product.edges,
        "acceptance_cells": limits.product.acceptance_cells,
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
            fp_given: true,
            workers_requested: 4,
            temporal: TemporalMode::FinalShort,
            phase,
        }
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
}
