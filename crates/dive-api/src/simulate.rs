//! Finite seeded safety simulation with independent trace validation.

use crate::trace::{Ending, SpecModel, TraceDoc, TraceOptions};
use dive_sim::{Options, Outcome};
use dive_stdlib::random::RandomMode;
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::PathBuf;

/// Version of simulation reports, separate from exhaustive checking reports.
pub const SIMULATE_SCHEMA_VERSION: u32 = 1;

/// A finite simulation request. The caller must choose a positive trace count.
///
/// ```
/// let request = dive_api::SimulateRequest {
///     file: "Counter.tla".into(), seed: Some(7), traces: 10,
///     depth: Some(100), ..Default::default()
/// };
/// assert_eq!(request.traces, 10);
/// ```
#[derive(Clone, Debug, Default)]
pub struct SimulateRequest {
    /// Root specification.
    pub file: PathBuf,
    /// Configuration; defaults to the specification's `.cfg` sibling.
    pub config: Option<PathBuf>,
    /// TLC's selection and value profile.
    pub compat_tlc: bool,
    /// Run seed; generated and reported when omitted.
    pub seed: Option<i64>,
    /// TLC's simulation seed adjustment; zero outside TLC mode.
    pub aril: u64,
    /// Maximum transitions per trace; `None` selects TLC's depth 100.
    pub depth: Option<usize>,
    /// Positive, finite number of traces, required by the API and CLI.
    pub traces: u64,
    /// Requested workers; only one is supported in this initial slice.
    pub workers: Option<usize>,
    /// TLC's `-deadlock`: disable deadlock checking.
    pub no_deadlock: bool,
    /// Collect source-location action flows, corresponding to TLC `stats=basic`.
    pub action_stats: bool,
    /// Modules searched before standard modules.
    pub library_paths: Vec<PathBuf>,
    /// Modules searched after standard modules.
    pub module_paths: Vec<PathBuf>,
}

/// Concrete sampled traces and independently checked violation evidence.
#[derive(Debug)]
pub struct SimulateReport {
    document: Value,
    exit: u8,
}

impl SimulateReport {
    /// Versioned JSON. `exhaustive` and `model_proved` are always false.
    #[must_use]
    pub fn to_json(&self) -> Value {
        self.document.clone()
    }

    /// Profile-specific exit status; sampling without an error exits zero.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        self.exit
    }

    /// Human-readable sampling summary that never claims a proof.
    #[must_use]
    pub fn render_human(&self) -> String {
        let mut text = format!(
            "simulation: {} ({} traces, {} generated states; no exhaustive proof)\nseed: {}\n",
            self.document["result"].as_str().unwrap_or("internal-error"),
            self.document["statistics"]["traces_started"],
            self.document["statistics"]["generated"],
            self.document["options"]["seed"]
        );
        if let Some(transitions) = self.document["action_flow"]["transitions"].as_array() {
            text.push_str("action flow (chosen transitions):\n");
            for edge in transitions {
                let _ = writeln!(
                    text,
                    "  group {} -> group {}: {}",
                    edge["from"], edge["to"], edge["count"]
                );
            }
        }
        text
    }
}

/// Loads and samples a specification; validates every returned trace separately.
///
/// # Errors
/// Unreadable/rejected files, unimplemented binding, or worker-thread failure.
#[allow(
    clippy::too_many_lines,
    reason = "report construction keeps the simulation and independent validation evidence together"
)]
pub fn simulate_file(req: &SimulateRequest) -> Result<SimulateReport, crate::ApiError> {
    let seed = req.seed.unwrap_or_else(dive_stdlib::random::fresh_seed);
    let options = Options::new(
        seed,
        if req.compat_tlc {
            RandomMode::Tlc
        } else {
            RandomMode::Dive
        },
        req.depth.unwrap_or(100),
        req.traces,
    );
    let options = Options {
        action_stats: req.action_stats,
        aril: if req.compat_tlc && req.seed.is_none() {
            0
        } else {
            req.aril
        },
        ..options
    };
    let config = req
        .config
        .clone()
        .unwrap_or_else(|| req.file.with_extension("cfg"));
    let model = SpecModel::load(
        &req.file,
        &config,
        &TraceOptions {
            compat_tlc: req.compat_tlc,
            no_deadlock: req.no_deadlock,
            library_paths: req.library_paths.clone(),
            module_paths: req.module_paths.clone(),
            ..TraceOptions::default()
        },
    )?;
    let result = if req.workers.is_some_and(|n| n != 1) {
        dive_sim::Report {
            stage: "preflight",
            outcome: Outcome::Unsupported("parallel simulation workers".to_owned()),
            generated: 0,
            initial_candidates: 0,
            pending_violation: None,
            traces_started: 0,
            traces: Vec::new(),
            action_flow: None,
        }
    } else {
        let executed = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .stack_size(dive_eval::RECOMMENDED_STACK)
                .spawn_scoped(scope, || dive_sim::simulate(model.model(), &options))
                .map_err(|e| (0_u8, format!("cannot start simulation worker: {e}")))?
                .join()
                .map_err(|_| {
                    (
                        1_u8,
                        "simulation worker failed internally; statistics unavailable".to_owned(),
                    )
                })
        });
        match executed {
            Ok(result) => result,
            Err((workers_used, message)) => {
                return Ok(SimulateReport {
                    exit: 4,
                    document: json!({
                        "schema_version": SIMULATE_SCHEMA_VERSION, "command": "simulate",
                        "file": req.file.display().to_string(), "config": config.display().to_string(),
                        "profile": if req.compat_tlc { "tlc" } else { "dive" },
                        "result": "internal-error", "exhaustive": false, "model_proved": false,
                        "message": message, "exit_code": 4, "violation_validated": false,
                        "options": { "seed": seed, "seed_given": req.seed.is_some(),
                            "aril": options.aril, "aril_requested": req.aril,
                            "action_stats": req.action_stats,
                            "depth": options.depth, "traces": options.traces,
                            "workers_requested": req.workers.unwrap_or(1), "workers_used": workers_used,
                            "execution_stage": if workers_used == 0 { "worker-start" } else { "worker-failure" } },
                        "statistics": { "scope": "unavailable", "generated": null, "traces_started": null },
                        "traces": [],
                    }),
                });
            }
        }
    };
    let (kind, code, message, default_exit) = match &result.outcome {
        Outcome::Sampled => ("sampled", None, None, 0),
        Outcome::Violation { code, .. } => (
            "violation",
            Some(*code),
            None,
            if *code == 2114 { 11 } else { 10 },
        ),
        Outcome::Error { code, message } => {
            ("evaluation-error", Some(*code), Some(message.clone()), 13)
        }
        Outcome::Unsupported(s) => ("unsupported", None, Some(s.clone()), 3),
        Outcome::Limit(s) => ("resource-limit", None, Some(s.clone()), 3),
        Outcome::Internal(s) => ("internal-error", None, Some(s.clone()), 4),
    };
    let mut traces = Vec::new();
    let mut trace_refusal = None;
    'documents: for (number, trace) in result.traces.iter().enumerate() {
        let states: Result<Vec<_>, _> = trace
            .states
            .iter()
            .map(|state| {
                let mut object = serde_json::Map::new();
                for (var, value) in model.model().ir.vars().iter().zip(state.iter()) {
                    let value = value
                        .as_ref()
                        .ok_or_else(|| "incomplete sampled trace".to_owned())?;
                    object.insert(
                        model.model().ir.decl(*var).name.clone(),
                        dive_trace::value_json::to_json(
                            value,
                            model.model().evaluator.options().ctx,
                        )?,
                    );
                }
                Ok::<_, String>(Value::Object(object))
            })
            .collect();
        let states = match states {
            Ok(states) => states,
            Err(feature) => {
                trace_refusal = Some(format!(
                    "simulation concrete trace serialization: {feature}"
                ));
                break;
            }
        };
        let ending = match &result.outcome {
            Outcome::Violation {
                kind, name, trace, ..
            } if *trace == number => match *kind {
                "invariant" => Ending::Invariant(name.clone().unwrap_or_default()),
                "property" => Ending::Property(name.clone().unwrap_or_default()),
                _ => Ending::Deadlock,
            },
            _ => Ending::Behavior,
        };
        let document = TraceDoc::behavior(states)
            .map_err(|message| crate::ApiError::Rejected { message })?
            .with_ending(ending);
        // Validation uses predicates and finite covers, not the simulation enumerator.
        let validation = model.validate(&document);
        let mut compressed = Vec::<usize>::new();
        for (index, state) in trace.states.iter().enumerate() {
            let same = if let Some(&previous) = compressed.last() {
                let mut equal = true;
                for (left, right) in trace.states[previous].iter().zip(state.iter()) {
                    if let (Some(left), Some(right)) = (left, right) {
                        match model.model().evaluator.equal_values(left, right) {
                            Ok(same) => equal &= same,
                            Err(error) => {
                                trace_refusal =
                                    Some(format!("simulation trace compression: {error}"));
                                break 'documents;
                            }
                        }
                    }
                }
                equal
            } else {
                false
            };
            if !same {
                compressed.push(index);
            }
        }
        let tlc_states: Vec<_> = compressed
            .iter()
            .map(|&i| document.states[i].clone())
            .collect();
        traces.push(json!({ "id": trace.id, "document": document.to_json(), "tlc_states": tlc_states, "validation": crate::trace::report_json(&validation) }));
    }
    let validation_failed = traces
        .iter()
        .any(|t| t["validation"]["result"] == "invalid");
    let violation_validated = match &result.outcome {
        Outcome::Violation { trace, .. } => traces
            .get(*trace)
            .is_some_and(|t| t["validation"]["result"] == "valid"),
        _ => false,
    };
    let undecidable = trace_refusal.is_some()
        || (matches!(result.outcome, Outcome::Violation { .. }) && !violation_validated)
        || (matches!(result.outcome, Outcome::Sampled)
            && traces.iter().any(|t| t["validation"]["result"] != "valid"));
    let (reported_kind, reported_code, reported_message) = if validation_failed {
        (
            "internal-error",
            None,
            Some("independent simulation trace validation rejected the candidate".to_owned()),
        )
    } else if let Some(feature) = trace_refusal {
        ("unsupported", None, Some(feature))
    } else if undecidable {
        let reasons: Vec<_> = traces
            .iter()
            .filter_map(|t| t["validation"]["unsupported"].as_str())
            .collect();
        (
            "unsupported",
            None,
            Some(format!(
                "independent simulation trace validation: {}",
                reasons.join("; ")
            )),
        )
    } else {
        (kind, code, message)
    };
    let exit = if validation_failed {
        4
    } else if undecidable {
        3
    } else if req.compat_tlc {
        code.map_or(default_exit, tlc_exit)
    } else {
        default_exit
    };
    let startup: Vec<_> = model
        .model()
        .output
        .iter()
        .map(|m| match m {
            dive_model::Message::Print(text) => json!({"kind": "print", "text": text}),
            dive_model::Message::Warning { code, message } => {
                json!({"kind": "warning", "code": code.code(), "text": message})
            }
        })
        .collect();
    let violation = match &result.outcome {
        Outcome::Violation {
            kind, name, trace, ..
        } => Some(json!({"kind": kind, "name": name, "trace": trace})),
        _ => None,
    };
    let pending_violation = result.pending_violation.as_ref().map(|pending| {
        json!({"kind": pending.kind, "name": pending.name, "tlc_code": pending.code,
            "validation": "unavailable", "reason": "complete trace exceeds retained-state bound"})
    });
    let action_flow = result.action_flow.as_ref().map(|flow| {
        let actions: Vec<_> = flow.actions.iter().map(|action| json!({
            "id": action.id, "name": action.name, "label": action.label,
            "initial": action.initial, "bound_action_ids": action.bound_action_ids,
            "location": {"module": action.location.file.map(|f| &model.model().ir.files()[f.index()].module),
                "begin": action.location.begin, "end": action.location.end},
        })).collect();
        let transitions: Vec<_> = flow.transitions.iter().map(|(&(from, to), &count)| {
            json!({"from": from, "to": to, "count": count})
        }).collect();
        json!({"mode": "basic", "scope": "chosen-transitions", "actions": actions,
            "transitions": transitions})
    });
    let document = json!({
        "schema_version": SIMULATE_SCHEMA_VERSION, "command": "simulate",
        "file": req.file.display().to_string(), "config": config.display().to_string(),
        "profile": if req.compat_tlc { "tlc" } else { "dive" },
        "result": reported_kind, "observed_result": kind, "observed_tlc_code": code, "exhaustive": false, "model_proved": false,
        "tlc_code": reported_code, "message": reported_message, "exit_code": exit,
        "violation_validated": violation_validated, "observed_violation": violation, "pending_violation": pending_violation, "startup_output": startup,
        "options": { "seed": seed, "seed_given": req.seed.is_some(), "aril": options.aril, "aril_requested": req.aril,
            "depth": options.depth, "traces": options.traces, "workers_requested": req.workers.unwrap_or(1),
            "workers_used": u8::from(result.stage != "preflight"),
            "execution_stage": result.stage, "execution_scope": "simulation-backend",
            "mode": "finite-safety", "probabilistic": false, "action_stats": req.action_stats },
        "statistics": { "generated": result.generated, "initial_candidates": result.initial_candidates, "traces_started": result.traces_started,
            "scope": if matches!(result.outcome, Outcome::Sampled) { "complete-request" } else { "completed-prefix" } },
        "traces": traces, "action_flow": action_flow,
    });
    Ok(SimulateReport { document, exit })
}

fn tlc_exit(code: u32) -> u8 {
    match code {
        2107 | 2110 => 12,
        2112 | 2113 => 13,
        2114 => 11,
        2104 | 2105 => 10,
        2109 => 75,
        2111 => 76,
        _ => 255,
    }
}
