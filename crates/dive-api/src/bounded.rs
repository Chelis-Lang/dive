//! Bounded safety over paths of the configured Init and Next.

use std::path::PathBuf;
use std::time::Duration;

use dive_runtime::budget::Phase;
use dive_sim::Outcome;
use serde_json::{Value, json};

use crate::core_modes::bounded::{ResultKind, Run};
use crate::trace::{SpecModel, TraceOptions, Validation};
use crate::{ApiError, BudgetSetting};

/// Options for an exact, transition-bounded path check.
///
/// ```no_run
/// use dive_api::bounded::{Request, check_file};
/// let mut request = Request::default();
/// request.file = "Counter.tla".into();
/// request.path_bound = 5;
/// let report = check_file(&request)?;
/// assert_eq!(report.to_json()["path_bound"], 5);
/// # Ok::<(), dive_api::ApiError>(())
/// ```
#[derive(Clone, Debug, Default)]
#[non_exhaustive]
pub struct Request {
    /// Root TLA+ source.
    pub file: PathBuf,
    /// Configuration. Defaults to the source path with a `.cfg` extension.
    pub config: Option<PathBuf>,
    /// Number of explicit Next transitions to check from Init at depth zero.
    pub path_bound: usize,
    /// Runtime limit for exploration and its independent replay.
    pub time_limit: BudgetSetting<Duration>,
    /// Process-memory limit.
    pub max_memory: BudgetSetting<u64>,
    /// Original source and independent validation options.
    pub trace: TraceOptions,
}

/// The finite-path verdict, its explicit bound and replay evidence.
#[derive(Clone, Debug)]
pub struct Report {
    document: Value,
    exit: u8,
}

impl Report {
    /// A JSON report that never claims unbounded model safety.
    #[must_use]
    pub fn to_json(&self) -> &Value {
        &self.document
    }

    /// Zero for a completed bounded safety check, or a native dive status.
    #[must_use]
    pub const fn exit_status(&self) -> u8 {
        self.exit
    }

    /// The result, bound, and any identified property or error.
    #[must_use]
    pub fn render_human(&self) -> String {
        let mut summary = format!(
            "Bounded path check through {} Next transitions: {}",
            self.document["path_bound"], self.document["result"]
        );
        if let Some(detail) = self.document["message"]
            .as_str()
            .or_else(|| self.document["property"].as_str())
        {
            summary.push_str(" (");
            summary.push_str(detail);
            summary.push(')');
        }
        summary.push('\n');
        summary
    }
}

/// Checks every reachable prefix through the bound. A repeated state at
/// another depth is a separate path position. Finite rejected prefixes
/// cannot be lengthened with invented stutters.
///
/// # Errors
/// Loading, budget setup or internal worker infrastructure failed.
#[allow(
    clippy::too_many_lines,
    reason = "the bound, shared budget, and replay determine a single published report"
)]
pub fn check_file(request: &Request) -> Result<Report, ApiError> {
    let config = request
        .config
        .clone()
        .unwrap_or_else(|| request.file.with_extension("cfg"));
    let source = SpecModel::load(&request.file, &config, &request.trace)?;
    let model = source.model();
    let assumptions = model
        .assumptions
        .iter()
        .map(|id| &model.ir.def(*id).name)
        .collect::<Vec<_>>();
    let invariants = model
        .invariants
        .iter()
        .map(|inv| &inv.name)
        .collect::<Vec<_>>();
    let mut document = json!({
        "schema_version": crate::REPORT_SCHEMA_VERSION, "command": "check", "check_kind": "bounded-path",
        "file": request.file, "config": config, "path_bound": request.path_bound,
        "model_proved": false, "exhaustive": false, "violation_validated": false,
        "path_exists_at_bound": null, "assumptions": assumptions, "invariants": invariants,
        "counterexample": null, "options": {"stuttering": "explicit-next-only"},
    });
    if let Some(feature) = refusal(&source, request) {
        document["result"] = json!("unsupported");
        document["message"] = json!(feature);
        document["exit_code"] = json!(3);
        return Ok(Report { document, exit: 3 });
    }
    let mut budget = crate::mode_budget::ModeBudget::start(
        Phase::Search,
        request.time_limit,
        request.max_memory,
    )?;
    let control = budget.control()?;
    let mut run = Run::new();
    let result = crate::trace::on_big_stack(|| {
        let _guard = control
            .enter(&model.evaluator)
            .map_err(|error| ApiError::Rejected {
                message: error.to_string(),
            })?;
        Ok::<_, ApiError>(run.check(
            model,
            request.path_bound,
            &budget.budget,
            source.config().specification.is_some(),
        ))
    })??;
    document["statistics"] = json!({
        "generated": run.generated, "checked_layers": run.checked_layers,
        "terminal_sources": run.terminal_sources,
    });
    if let Some(initial_query) = run.initial_query {
        document["initial_query"] = initial_query;
    }
    let (kind, message, mut exit) = match result {
        ResultKind::ReachedBound => {
            document["path_exists_at_bound"] = json!(true);
            ("bounded-safe", None, 0)
        }
        ResultKind::ExecutionsTooShort => {
            document["path_exists_at_bound"] = json!(false);
            ("executions-too-short", None, 0)
        }
        ResultKind::Failed(Outcome::Violation { kind, name, .. }) => {
            match crate::trace::finite_witness(
                &source,
                &run.witness,
                kind,
                name.as_deref(),
                &budget.budget,
                control,
            ) {
                Ok((trace, validation)) if validation.validation == Validation::Valid => {
                    document["counterexample"] = json!({"trace": trace.to_json()});
                    document["violation_validated"] = json!(true);
                    document["property"] = json!(name);
                    document["violation_depth"] = json!(run.witness.len().saturating_sub(1));
                    ("invariant-violation", None, 10)
                }
                Ok((_, validation)) => (
                    "unsupported",
                    Some(format!(
                        "independent bounded path replay: {:?}",
                        validation.validation
                    )),
                    3,
                ),
                Err(_) if budget.budget.poll().is_ok_and(|stop| stop.is_some()) => {
                    ("stopped", None, 5)
                }
                Err(message) => ("unsupported", Some(message), 3),
            }
        }
        ResultKind::Failed(Outcome::Error {
            code: 2104,
            message,
        }) => ("assumption-failure", Some(message), 12),
        ResultKind::Failed(Outcome::Error { message, .. }) => {
            ("evaluation-error", Some(message), 13)
        }
        ResultKind::Failed(Outcome::Unsupported(message) | Outcome::Limit(message)) => {
            ("unsupported", Some(message), 3)
        }
        ResultKind::Failed(Outcome::Cancelled) => ("stopped", None, 5),
        ResultKind::Failed(Outcome::Internal(message)) => ("internal-error", Some(message), 4),
        ResultKind::Failed(Outcome::Sampled) => (
            "internal-error",
            Some("sampled bounded path result".into()),
            4,
        ),
    };
    document["result"] = json!(kind);
    document["message"] = json!(message);
    document["exit_code"] = json!(exit);
    budget.finish(&mut document, &mut exit, kind == "stopped")?;
    if exit == 5 {
        document["path_exists_at_bound"] = Value::Null;
        document["property"] = Value::Null;
        document["violation_depth"] = Value::Null;
    }
    Ok(Report { document, exit })
}

fn refusal(source: &SpecModel, request: &Request) -> Option<String> {
    let model = source.model();
    let cfg = source.config();
    if let Some(feature) = model.unsupported.first() {
        return Some(feature.feature.clone());
    }
    [
        (request.trace.compat_tlc, "the TLC compatibility profile"),
        (
            model.init.is_empty() || model.next.is_none(),
            "configurations without bound Init and Next",
        ),
        (!cfg.properties.is_empty(), "PROPERTY"),
        (
            !model.temporals.is_empty(),
            "temporal specification assumptions",
        ),
        (cfg.symmetry.is_some(), "SYMMETRY"),
        (cfg.view.is_some(), "VIEW"),
        (cfg.alias.is_some(), "ALIAS"),
        (!cfg.postconditions.is_empty(), "POSTCONDITION"),
        (
            cfg.check_deadlock.as_ref().is_some_and(|flag| flag.value),
            "explicit CHECK_DEADLOCK TRUE",
        ),
        (
            cfg.periodic.is_some() || cfg.rl_reward.is_some() || !cfg.possible.is_empty(),
            "runtime CFG extensions",
        ),
        (
            !model.output.is_empty() || dive_runtime::admission::effectful(model),
            "effects and startup output",
        ),
    ]
    .into_iter()
    .find(|(active, _)| *active)
    .map(|(_, feature)| format!("bounded path checking refuses {feature}"))
}
