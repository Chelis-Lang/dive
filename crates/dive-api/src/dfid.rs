//! Native depth-first iterative deepening for Rust callers (ADR 0021).
//!
//! ```no_run
//! use dive_api::{DfidRequest, dfid_file};
//! let request = DfidRequest {
//!     file: "Counter.tla".into(), max_level: 8, seed: Some(7),
//!     ..DfidRequest::default()
//! };
//! let evidence = dfid_file(&request)?.to_json();
//! // A zero exit or a finished iteration is not a model-safety certificate.
//! let proved = evidence["model_proved"].as_bool().unwrap_or(false);
//! # let _ = proved;
//! # Ok::<(), dive_api::ApiError>(())
//! ```

use crate::dfid_admission::{Options, Outcome};
use crate::trace::{Ending, SpecModel, TraceDoc, TraceOptions, Validation};
use serde_json::{Value, json};
use std::path::PathBuf;

/// Version of the DFID JSON report.
pub const DFID_SCHEMA_VERSION: u32 = crate::REPORT_SCHEMA_VERSION;

/// Finite DFID request. Depth exhaustion cannot establish model safety.
#[derive(Clone, Debug, Default)]
pub struct DfidRequest {
    /// Root specification file.
    pub file: PathBuf,
    /// Model configuration; defaults to the specification's `.cfg`.
    pub config: Option<PathBuf>,
    /// TLC profile (fresh per-iteration worker seeds unless explicitly replayed).
    pub compat_tlc: bool,
    /// Maximum iterative level, corresponding to TLC `-dfid N`.
    pub max_level: u32,
    /// Reported run seed; native workers derive replay streams from it.
    pub seed: Option<i64>,
    /// Fingerprint polynomial, default zero.
    pub fp: usize,
    /// Explicit selection seed per iteration. TLC profile becomes mixed.
    pub worker_seeds: Option<Vec<i64>>,
    /// Requested worker count, currently one only.
    pub workers: Option<usize>,
    /// Exact canonical store instead of fingerprints with collision guard.
    pub exact: bool,
    /// Disable deadlock checking, matching TLC `-deadlock`.
    pub no_deadlock: bool,
    /// Time limit; the profile default matches `dive check`. `Unlimited` is TLC-equivalent.
    pub time_limit: crate::BudgetSetting<std::time::Duration>,
    /// Sampled process-memory ceiling; the default is half physical memory.
    pub max_memory: crate::BudgetSetting<u64>,
    /// Modules searched before the standard modules.
    pub library_paths: Vec<PathBuf>,
    /// Additional module directories.
    pub module_paths: Vec<PathBuf>,
}

/// Structured evidence and process disposition for a DFID request.
#[derive(Clone, Debug)]
pub struct DfidReport {
    document: Value,
    exit: u8,
}

impl DfidReport {
    /// Report with explicit completion, execution, and validation evidence.
    #[must_use]
    pub fn to_json(&self) -> Value {
        self.document.clone()
    }
    /// CLI disposition; zero in TLC mode may mean a depth-bound stop.
    #[must_use]
    pub const fn exit_status(&self) -> u8 {
        self.exit
    }
    /// Concise human description with the completion claim stated explicitly.
    /// A refusal, limit or error names its reason, as in the JSON `message`.
    #[must_use]
    pub fn render_human(&self) -> String {
        let mut text = format!(
            "DFID: {}; model proved: {}; generated {}, distinct {}; seed {}\n",
            self.document["result"],
            self.document["model_proved"],
            self.document["statistics"]["generated"],
            self.document["statistics"]["distinct"],
            self.document["options"]["seed"]
        );
        if let (Some(result), Some(message)) = (
            self.document["result"].as_str(),
            self.document["message"].as_str(),
        ) {
            text.push_str(result);
            text.push_str(": ");
            text.push_str(message);
            text.push('\n');
        }
        text
    }
}

/// Loads and runs a DFID model on the default engine (Core), independently
/// certifying every violation.
///
/// # Errors
/// This API retains load and thread errors as structured unknown/partial
/// evidence. The `Result` is retained for consistency with the other APIs.
#[allow(
    clippy::too_many_lines,
    reason = "keep report provenance and certification in one path"
)]
pub fn dfid_file(req: &DfidRequest) -> Result<DfidReport, crate::ApiError> {
    dfid_file_engine(req, crate::Engine::default())
}

/// Runs DFID on the given engine, Core (the only engine), with common
/// independent witnesses. A reached refusal stops; there is no fallback.
/// `Reference` runs the reference DFID backend.
///
/// # Errors
/// The same budget and ownership failures as [`dfid_file`].
#[allow(
    clippy::too_many_lines,
    reason = "engine selection and independent certification share one DFID report path"
)]
pub fn dfid_file_engine(
    req: &DfidRequest,
    engine: crate::Engine,
) -> Result<DfidReport, crate::ApiError> {
    let seed = req.seed.unwrap_or_else(dive_stdlib::random::fresh_seed);
    let config = req
        .config
        .clone()
        .unwrap_or_else(|| req.file.with_extension("cfg"));
    let profile = if req.compat_tlc {
        if req.worker_seeds.is_some() || matches!(req.time_limit, crate::BudgetSetting::Explicit(_))
        {
            "tlc+overrides"
        } else {
            "tlc"
        }
    } else {
        "dive"
    };
    let mut document = json!({
        "schema_version": DFID_SCHEMA_VERSION, "command": "dfid", "profile": profile,
        "engine_requested": engine.name(), "engine": "none", "engine_fallback": null,
        "file": req.file.display().to_string(), "config": config.display().to_string(),
        "exhaustive": false, "model_proved": false, "violation_validated": false,
        "result": "internal-error", "termination": "unavailable", "tlc_code": null,
        "options": {"seed": seed, "seed_given": req.seed.is_some(), "fp": req.fp,
            "max_level": req.max_level, "workers_requested": req.workers.unwrap_or(1), "workers_used": 0,
            "worker_seeds": [], "worker_seed_override": req.worker_seeds,
            "cli_seed_controls_workers": !req.compat_tlc, "store": if req.exact { "exact" } else { "fingerprint+exact-collision-guard" },
            "execution_stage": "model-loading", "execution_scope": "native-dfid",
            "witness_policy": "actual-concrete-path (D-019)"},
        "statistics": {"scope": "unavailable", "generated": null, "distinct": null, "level": null, "initial_candidates": null},
        "counterexample": null, "startup_output": null,
    });
    // TLC rejects a zero width while parsing arguments, before opening a model.
    if req.workers == Some(0) {
        let exit = if req.compat_tlc { 1 } else { 2 };
        document["result"] = json!("parameter-error");
        document["observed_result"] = json!("parameter-error");
        document["observed_tlc_code"] = json!(1102);
        document["tlc_code"] = json!(if req.compat_tlc { Some(1102) } else { None });
        document["message"] = json!("Error: at least one worker required.");
        document["termination"] = json!("parameter-validation");
        document["options"]["execution_stage"] = json!("parameter-validation");
        document["exit_code"] = json!(exit);
        return Ok(DfidReport { document, exit });
    }
    let model = match SpecModel::load(
        &req.file,
        &config,
        &TraceOptions {
            inputs: None,
            compat_tlc: req.compat_tlc,
            symmetry_liveness: None,
            no_deadlock: req.no_deadlock,
            run: Some((seed, req.fp)),
            library_paths: req.library_paths.clone(),
            module_paths: req.module_paths.clone(),
        },
    ) {
        Ok(model) => model,
        Err(e) => {
            let (kind, exit) = match e {
                crate::ApiError::Unsupported { .. } => ("unsupported", 3),
                crate::ApiError::Rejected { .. } => ("rejected", 1),
                crate::ApiError::Io { .. } => ("input-error", 2),
                crate::ApiError::BudgetStartup(_) | crate::ApiError::BudgetMeasurement(_) => {
                    ("internal-error", 4)
                }
            };
            document["result"] = json!(kind);
            document["message"] = json!(e.to_string());
            document["exit_code"] = json!(exit);
            return Ok(DfidReport { document, exit });
        }
    };
    document["startup_output"] = json!(
        model
            .model()
            .output
            .iter()
            .map(|m| match m {
                dive_model::Message::Print(text) => json!({"kind": "print", "text": text}),
                dive_model::Message::Warning { code, message } =>
                    json!({"kind": "warning", "code": code.code(), "text": message}),
            })
            .collect::<Vec<_>>()
    );
    let mut run_budget = crate::mode_budget::ModeBudget::start(
        dive_runtime::budget::Phase::Dfid,
        req.time_limit,
        req.max_memory,
    )?;
    let control = run_budget.control()?;
    let control_scope =
        control
            .enter(&model.model().evaluator)
            .map_err(|error| crate::ApiError::Rejected {
                message: error.to_string(),
            })?;
    let mut check = if req.compat_tlc {
        dive_runtime::CheckOptions::tlc(req.fp, seed)
    } else {
        dive_runtime::CheckOptions::dive(seed)
    };
    check.budget = run_budget.budget.clone();
    check.fp_index = req.fp;
    check.workers = req.workers.unwrap_or(1);
    check.no_deadlock = req.no_deadlock;
    if req.exact {
        check.store = dive_store::StoreMode::Exact;
    }
    let mut options = Options::new(check, req.max_level);
    options.worker_seeds.clone_from(&req.worker_seeds);
    let mut native = None;
    let mut setup_failure = None;
    let crate::Engine::Core = engine;
    {
        if let Some(outcome) = crate::dfid_admission::preflight(model.model(), &options) {
            setup_failure = Some(outcome);
        } else {
            match crate::core_modes::Native::probe(
                &model,
                seed,
                if req.compat_tlc {
                    dive_stdlib::random::RandomMode::Tlc
                } else {
                    dive_stdlib::random::RandomMode::Dive
                },
                &run_budget.budget,
                control.clone(),
            ) {
                Ok(prepared) => native = Some(prepared),
                Err(error) => setup_failure = Some(error.dfid()),
            }
        }
    }
    document["engine"] = json!(engine.name());
    // Retained in report schema 3; there is no automatic fallback.
    document["engine_fallback"] = Value::Null;
    document["options"]["execution_scope"] = json!(if engine == crate::Engine::Core {
        "compiled-plan-dfid"
    } else {
        "native-dfid"
    });
    let executed = std::thread::scope(|scope| {
        std::thread::Builder::new()
            .stack_size(dive_eval::RECOMMENDED_STACK)
            .spawn_scoped(scope, || {
                if let Some(failure) = setup_failure {
                    crate::core_modes::dfid::empty(failure)
                } else if let Some(native) = &mut native {
                    native.dfid(model.model(), &options)
                } else {
                    crate::core_modes::dfid::empty(Outcome::Internal(
                        "Core DFID was neither prepared nor refused".to_owned(),
                    ))
                }
            })
            .map_err(|e| (0u8, format!("cannot start DFID worker: {e}")))?
            .join()
            .map_err(|_| {
                (
                    1u8,
                    "DFID worker failed internally; statistics unavailable".to_owned(),
                )
            })
    });
    let report = match executed {
        Ok(report) => report,
        Err((workers, message)) => {
            document["options"]["workers_used"] = json!(workers);
            document["options"]["execution_stage"] = json!(if workers == 0 {
                "worker-start"
            } else {
                "worker-failure"
            });
            document["message"] = json!(message);
            document["exit_code"] = json!(4);
            return Ok(DfidReport { document, exit: 4 });
        }
    };
    let (mut kind, code, mut message, mut exit) = match &report.outcome {
        Outcome::Complete => ("complete", None, None, 0),
        Outcome::Incomplete => (
            "incomplete",
            None,
            Some("DFID iterative depth bound reached without completion".to_owned()),
            if req.compat_tlc {
                if req.max_level < 2 { 255 } else { 0 }
            } else {
                3
            },
        ),
        Outcome::Violation { code, .. } => (
            "violation",
            Some(*code),
            None,
            if req.compat_tlc {
                tlc_exit(*code)
            } else if *code == 2114 {
                11
            } else {
                10
            },
        ),
        Outcome::Error { code, message } => (
            "evaluation-error",
            Some(*code),
            Some(message.clone()),
            if req.compat_tlc { tlc_exit(*code) } else { 13 },
        ),
        Outcome::Unsupported(s) => ("unsupported", None, Some(s.clone()), 3),
        Outcome::Limit(s) => ("resource-limit", None, Some(s.clone()), 3),
        Outcome::Cancelled => (
            "resource-limit",
            None,
            Some("DFID run budget exhausted".to_owned()),
            3,
        ),
        Outcome::Internal(s) => ("internal-error", None, Some(s.clone()), 4),
    };
    let observed_kind = kind;
    let mut violation_validated = false;
    if let Outcome::Violation {
        kind: violation_kind,
        name,
        ..
    } = &report.outcome
    {
        match witness(
            &model,
            &report.trace,
            violation_kind,
            name.as_deref(),
            &run_budget.budget,
            control.clone(),
        ) {
            Ok((trace, validation)) => {
                document["counterexample"] = json!({"trace": trace.to_json(), "validation": crate::trace::report_json(&validation)});
                match validation.validation {
                    Validation::Valid => violation_validated = true,
                    Validation::Invalid(_) => {
                        kind = "internal-error";
                        exit = 4;
                        message = Some(
                            "independent DFID witness validation rejected the candidate".to_owned(),
                        );
                    }
                    Validation::Unsupported(feature) => {
                        kind = "unsupported";
                        exit = 3;
                        message = Some(format!("independent DFID witness validation: {feature}"));
                    }
                }
            }
            Err(feature) => {
                kind = "unsupported";
                exit = 3;
                message = Some(feature);
            }
        }
    }
    document["result"] = json!(kind);
    document["observed_result"] = json!(observed_kind);
    document["observed_deciding"] = json!(report.deciding.as_ref().map(|o| format!("{o:?}")));
    document["observed_tlc_code"] = json!(code);
    document["tlc_code"] = json!(if observed_kind == kind { code } else { None });
    document["message"] = json!(message);
    document["exit_code"] = json!(exit);
    document["violation_validated"] = json!(violation_validated);
    document["exhaustive"] = json!(matches!(report.outcome, Outcome::Complete));
    document["model_proved"] = json!(matches!(report.outcome, Outcome::Complete));
    document["termination"] = json!(if matches!(report.outcome, Outcome::Complete) {
        "visitation-certificate"
    } else {
        report.stage
    });
    document["tlc_completion_message"] = json!(report.tlc_completion_message);
    document["options"]["workers_used"] = json!(u8::from(report.stage != "preflight"));
    document["options"]["execution_stage"] = json!(report.stage);
    document["options"]["wrapper_threads_used"] = json!(1);
    document["compatibility_claim"] = json!(
        "documented pure safety search fields only; raw display and collision probabilities pending"
    );
    document["options"]["worker_seeds"] = json!(report.worker_seeds);
    document["statistics"] = json!({"scope": if matches!(report.outcome, Outcome::Complete) {"complete-model"} else {"completed-prefix"}, "generated": report.generated, "distinct": report.distinct, "initial_candidates": report.initial_candidates, "level": report.level});
    drop(control_scope);
    run_budget.finish(
        &mut document,
        &mut exit,
        matches!(report.outcome, Outcome::Cancelled),
    )?;
    Ok(DfidReport { document, exit })
}

fn witness(
    model: &SpecModel,
    states: &[dive_eval::next::StateVals],
    kind: &str,
    name: Option<&str>,
    budget: &dive_runtime::budget::Budget,
    control: std::sync::Arc<dyn dive_eval::HostControl>,
) -> Result<(TraceDoc, crate::trace::Report), String> {
    let mut objects = Vec::new();
    for state in states {
        if budget.poll() != Ok(None) {
            return Err("run budget exhausted during DFID witness serialization".to_owned());
        }
        let mut object = serde_json::Map::new();
        for (var, value) in model.model().ir.vars().iter().zip(state.iter()) {
            let value = value
                .as_ref()
                .ok_or_else(|| "incomplete DFID witness state".to_owned())?;
            object.insert(
                model.model().ir.decl(*var).name.clone(),
                dive_trace::value_json::to_json(value, model.model().evaluator.options().ctx)?,
            );
        }
        objects.push(Value::Object(object));
    }
    let ending = match kind {
        "invariant" => Ending::Invariant(name.unwrap_or_default().to_owned()),
        "property" => Ending::Property(name.unwrap_or_default().to_owned()),
        _ => Ending::Deadlock,
    };
    let document = TraceDoc::behavior(objects)?.with_ending(ending);
    let validation = model
        .validate_mode(&document, control)
        .map_err(|error| error.to_string())?;
    Ok((document, validation))
}

fn tlc_exit(code: u32) -> u8 {
    match code {
        1102 => 1,
        2104 => 10,
        2114 => 11,
        2107 | 2110 => 12,
        2108 | 2112 => 13,
        2103 | 2109 => 75,
        _ => 255,
    }
}
