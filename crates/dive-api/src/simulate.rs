//! Finite seeded safety simulation with independent trace validation.

use crate::trace::{Ending, SpecModel, TraceDoc, TraceOptions};
use dive_sim::{KEEP_ALL_TRACES, Options, Outcome, UNLIMITED_TRACES};
use dive_stdlib::random::RandomMode;
use serde_json::{Value, json};
use std::fmt::Write as _;
use std::path::PathBuf;

/// Version of simulation reports, separate from exhaustive checking reports.
pub const SIMULATE_SCHEMA_VERSION: u32 = crate::REPORT_SCHEMA_VERSION;

/// A simulation request. The caller must choose a positive trace count, or
/// [`UNLIMITED_TRACES`] with an explicit time limit.
///
/// ```
/// let request = dive_api::SimulateRequest {
///     file: "Counter.tla".into(), seed: Some(7), traces: 10,
///     depth: Some(100), ..Default::default()
/// };
/// assert_eq!(request.traces, 10);
/// ```
#[derive(Clone, Debug, Default)]
#[allow(
    clippy::struct_excessive_bools,
    reason = "independent public simulation options retain their existing boolean API"
)]
pub struct SimulateRequest {
    /// Root specification.
    pub file: PathBuf,
    /// Configuration; defaults to the specification's `.cfg` sibling.
    pub config: Option<PathBuf>,
    /// TLC's value semantics; exact random draws are not a parity obligation.
    pub compat_tlc: bool,
    /// Run seed; generated and reported when omitted.
    pub seed: Option<i64>,
    /// TLC's simulation seed adjustment; zero outside TLC mode.
    pub aril: u64,
    /// Maximum transitions per trace; `None` selects TLC's depth 100.
    pub depth: Option<usize>,
    /// Positive total number of traces at any worker count, required by the
    /// API and CLI. [`UNLIMITED_TRACES`] samples until a violation, an error
    /// or the time limit, which must then be explicit.
    pub traces: u64,
    /// Sampling threads (default 1). The report is the same at any count.
    pub workers: Option<usize>,
    /// Traces kept, validated and reported besides the deciding trace: the
    /// lowest ids. `None` keeps 10; [`KEEP_ALL_TRACES`] keeps every trace.
    pub keep_traces: Option<u64>,
    /// Serialized report size the kept traces may fill: traces are admitted
    /// in id order while [`SimulateReport::write_json`] stays within it, and
    /// `statistics.traces_dropped` counts the others. A deciding trace that
    /// does not fit is replaced by a summary with its length, ending and the
    /// SHA-256 of its document. `None` admits every kept trace.
    pub max_report_bytes: Option<usize>,
    /// Each trace constructs its initial state instead of drawing one from
    /// Init enumerated once (D-041). Init is never enumerated, so a huge Init
    /// starts at once, but an initial state that violates an invariant is
    /// found only if a trace constructs it.
    pub constructive_init: bool,
    /// Sets Init or Next would enumerate but cannot because they are
    /// infinite (`Nat`, `Int`, `Seq(S)`, D-040). `None` selects the profile's
    /// default: geometric with mean [`crate::DEFAULT_SAMPLE_MEAN`] in dive's
    /// profile, [`crate::SampleUnbounded::Off`] (TLC's GENERAL 1000 error)
    /// under `compat_tlc`.
    pub sample_unbounded: Option<crate::SampleUnbounded>,
    /// Stop after this many generated states, counted in trace order.
    /// `None` leaves the run to its time and memory budgets.
    pub max_generated: Option<u64>,
    /// TLC's `-deadlock`: disable deadlock checking.
    pub no_deadlock: bool,
    /// Collect source-location action flows, corresponding to TLC `stats=basic`.
    pub action_stats: bool,
    /// Construct one successor per selected action instead of retaining all
    /// candidates. This is finite sampling, never exhaustive checking.
    /// `true` corresponds to TLC `-generate num=N`; `false` to `-simulate num=N`.
    pub generation: bool,
    /// Time limit; the profile default matches `dive check`. `Unlimited` is TLC-equivalent.
    pub time_limit: crate::BudgetSetting<std::time::Duration>,
    /// Sampled process-memory ceiling; the default is half physical memory.
    pub max_memory: crate::BudgetSetting<u64>,
    /// Modules searched before standard modules.
    pub library_paths: Vec<PathBuf>,
    /// Modules searched after standard modules.
    pub module_paths: Vec<PathBuf>,
    /// Extra steps a trace cut by the depth may take while a temporal
    /// property is unchecked on it (D-039). `None` is the profile default:
    /// the depth in the default profile, zero under `--compat=tlc`.
    pub liveness_extend: Option<usize>,
    /// Per-trace temporal work budget; `None` scales with the trace
    /// ([`dive_sim::liveness_budget`]). A budget stop is never a verdict.
    pub liveness_budget: Option<u64>,
}

/// Concrete sampled traces and independently checked violation evidence.
///
/// Each trace entry is kept as its compact serialized JSON rather than as a
/// JSON tree, which costs many times its text (issue #1024); the `traces`
/// key of `document` is an empty placeholder that keeps its key order.
#[derive(Debug)]
pub struct SimulateReport {
    document: Value,
    traces: Vec<String>,
    exit: u8,
}

impl SimulateReport {
    /// Versioned JSON. `exhaustive` and `model_proved` are always false.
    ///
    /// This builds every trace document as a JSON tree; [`Self::write_json`]
    /// writes the same text without building it.
    #[must_use]
    pub fn to_json(&self) -> Value {
        let mut document = self.document.clone();
        self.fill_traces(&mut document);
        document
    }

    fn fill_traces(&self, document: &mut Value) {
        if !self.traces.is_empty() {
            document["traces"] = Value::Array(
                self.traces
                    .iter()
                    .map(|entry| serde_json::from_str(entry).unwrap_or(Value::Null))
                    .collect(),
            );
        }
    }

    /// Bytes of the retained serialized trace entries: what the report holds
    /// per trace beyond its fixed summary.
    #[must_use]
    pub fn retained_trace_bytes(&self) -> usize {
        self.traces.iter().map(String::len).sum()
    }

    /// Writes the compact JSON text of [`Self::to_json`], or of
    /// [`Self::to_json_with_exit_convention`] when `convention` is given,
    /// without materializing the trace documents as a JSON tree. No newline
    /// is written.
    ///
    /// ```
    /// # fn show(report: &dive_api::SimulateReport) -> std::io::Result<()> {
    /// let mut text = Vec::new();
    /// report.write_json(&mut text, None)?;
    /// assert_eq!(String::from_utf8_lossy(&text), report.to_json().to_string());
    /// # Ok(()) }
    /// ```
    ///
    /// # Errors
    /// The writer's errors.
    pub fn write_json(
        &self,
        out: &mut dyn std::io::Write,
        convention: Option<crate::ExitConvention>,
    ) -> std::io::Result<()> {
        let header = match convention {
            Some(convention) => self.header_with_exit_convention(convention),
            None => self.document.clone(),
        };
        let Value::Object(map) = &header else {
            return write!(out, "{header}");
        };
        out.write_all(b"{")?;
        for (index, (key, value)) in map.iter().enumerate() {
            if index > 0 {
                out.write_all(b",")?;
            }
            serde_json::to_writer(&mut *out, key)?;
            out.write_all(b":")?;
            if key == "traces" && !self.traces.is_empty() {
                out.write_all(b"[")?;
                for (number, entry) in self.traces.iter().enumerate() {
                    if number > 0 {
                        out.write_all(b",")?;
                    }
                    out.write_all(entry.as_bytes())?;
                }
                out.write_all(b"]")?;
            } else {
                serde_json::to_writer(&mut *out, value)?;
            }
        }
        out.write_all(b"}")
    }

    /// Profile-specific exit status; sampling without an error exits zero.
    #[must_use]
    pub const fn exit_code(&self) -> u8 {
        self.exit
    }

    /// Process status under an explicit convention, independently of value
    /// semantics. Refusals, resource stops and internal errors keep their status.
    #[must_use]
    pub fn exit_code_for(&self, convention: crate::ExitConvention) -> u8 {
        let code = self.document["tlc_code"]
            .as_u64()
            .and_then(|code| u32::try_from(code).ok());
        match (self.document["result"].as_str(), code) {
            (Some("violation"), Some(code)) => match convention {
                crate::ExitConvention::Tlc => tlc_exit(code),
                crate::ExitConvention::Native => {
                    if code == 2114 {
                        11
                    } else {
                        10
                    }
                }
            },
            (Some("evaluation-error"), Some(code)) => match convention {
                crate::ExitConvention::Tlc => tlc_exit(code),
                crate::ExitConvention::Native => 13,
            },
            _ => self.exit,
        }
    }

    /// Versioned report with an explicitly selected process-status convention.
    /// The simulation remains non-exhaustive under either convention.
    #[must_use]
    pub fn to_json_with_exit_convention(&self, convention: crate::ExitConvention) -> Value {
        let mut document = self.header_with_exit_convention(convention);
        self.fill_traces(&mut document);
        document
    }

    fn header_with_exit_convention(&self, convention: crate::ExitConvention) -> Value {
        let mut document = self.document.clone();
        document["exit_code"] = json!(self.exit_code_for(convention));
        document["options"]["exit_codes"] = json!(match convention {
            crate::ExitConvention::Native => "native",
            crate::ExitConvention::Tlc => "tlc",
        });
        document
    }

    /// Human-readable sampling summary that never claims a proof.
    #[must_use]
    pub fn render_human(&self) -> String {
        let mut text = String::new();
        for field in ["startup_output", "runtime_output"] {
            if let Some(events) = self.document[field].as_array() {
                for event in events {
                    if let Some(line) = event["text"].as_str() {
                        text.push_str(line);
                        text.push('\n');
                    }
                }
            }
        }
        let _ = write!(
            text,
            "simulation: {} ({} traces, {} generated states; no exhaustive proof)\nseed: {}\n",
            self.document["result"].as_str().unwrap_or("internal-error"),
            self.document["statistics"]["traces_started"],
            self.document["statistics"]["generated"],
            self.document["options"]["seed"]
        );
        // A refusal, limit or error names its reason, as in the JSON `message`.
        if let (Some(result), Some(message)) = (
            self.document["result"].as_str(),
            self.document["message"].as_str(),
        ) {
            let _ = writeln!(text, "{result}: {message}");
        }
        for property in self.document["statistics"]["properties"]
            .as_array()
            .into_iter()
            .flatten()
        {
            let name = property["name"].as_str().unwrap_or_default();
            if let Some(trace) = property["refuted_by_trace"].as_u64() {
                let _ = writeln!(text, "{name}: violated on trace {trace}");
                continue;
            }
            let checked = property["checked_traces"].as_u64().unwrap_or(0);
            let unchecked = property["unchecked_traces"].as_u64().unwrap_or(0);
            if checked + unchecked == 0 {
                let _ = writeln!(text, "{name}: not checked on any completed trace");
                continue;
            }
            let _ = write!(
                text,
                "{name}: no counterexample on {checked} of {} traces",
                checked + unchecked
            );
            if unchecked > 0 {
                let reasons: Vec<String> = property["unchecked_reasons"]
                    .as_object()
                    .into_iter()
                    .flatten()
                    .map(|(reason, count)| format!("{count} {reason}"))
                    .collect();
                let _ = write!(
                    text,
                    "; {unchecked} traces could not decide it ({})",
                    reasons.join(", ")
                );
            }
            text.push_str(" (not a proof)\n");
        }
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
        if let Some(notes) = self.document["notes"].as_array() {
            for note in notes.iter().filter_map(Value::as_str) {
                let _ = writeln!(text, "note: {note}");
            }
        }
        text
    }
}

/// Loads and samples a specification with the compiled Core, and validates
/// every returned trace separately.
///
/// What Core does not implement is a named refusal, and a reached refusal
/// never reruns effects.
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
    if req.traces == UNLIMITED_TRACES
        && !matches!(req.time_limit, crate::BudgetSetting::Explicit(_))
    {
        return Err(crate::ApiError::Rejected {
            message: "unlimited simulation traces need an explicit time limit".to_owned(),
        });
    }
    let workers = req.workers.unwrap_or(1);
    let depth = options.depth;
    let options = Options {
        liveness_extend: req
            .liveness_extend
            .unwrap_or(if req.compat_tlc { 0 } else { depth }),
        liveness_budget: req.liveness_budget,
        bad_prefix: !req.compat_tlc,
        action_stats: req.action_stats,
        generation: req.generation,
        constructive_init: req.constructive_init,
        workers,
        keep_traces: req.keep_traces.unwrap_or(10),
        sample_unbounded: req.sample_unbounded.unwrap_or(if req.compat_tlc {
            crate::SampleUnbounded::Off
        } else {
            crate::SampleUnbounded::Geometric {
                mean: crate::DEFAULT_SAMPLE_MEAN,
            }
        }),
        max_generated: req.max_generated.unwrap_or(u64::MAX),
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
    let runtime = dive_sim::Runtime::new(options.clone());
    let model = SpecModel::load_hosted(
        &req.file,
        &config,
        &TraceOptions {
            compat_tlc: req.compat_tlc,
            no_deadlock: req.no_deadlock,
            run: Some((seed, 0)),
            library_paths: req.library_paths.clone(),
            module_paths: req.module_paths.clone(),
            ..TraceOptions::default()
        },
        Some(runtime.host()),
    )?;
    runtime.set_config(Some(crate::trace::make_run_config(model.model(), seed, 0)));
    let mut run_budget = crate::mode_budget::ModeBudget::start(
        dive_runtime::budget::Phase::Simulation,
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
    let mut native = None;
    let mut setup_failure = None;
    let mut symmetry_warnings = Vec::new();
    if let Some(outcome) = dive_sim::preflight(model.model(), &options) {
        setup_failure = Some(outcome);
    } else if let Err(outcome) = prepare_symmetry(&model, &mut symmetry_warnings) {
        setup_failure = Some(outcome);
    } else {
        match crate::core_modes::Native::probe(
            &model,
            seed,
            options.mode,
            &run_budget.budget,
            control.clone(),
        ) {
            Ok(prepared) => native = Some(prepared),
            Err(error) => setup_failure = Some(error.simulation()),
        }
    }
    let result = if let Some(outcome) = setup_failure {
        crate::core_modes::preflight_report(outcome)
    } else {
        let executed = std::thread::scope(|scope| {
            std::thread::Builder::new()
                .stack_size(dive_eval::RECOMMENDED_STACK)
                .spawn_scoped(scope, || {
                    let poll = || run_budget.budget.poll() != Ok(None);
                    if let Some(native) = &mut native {
                        native.simulate(model.model(), &options, &runtime, &poll)
                    } else {
                        crate::core_modes::preflight_report(Outcome::Internal(
                            "Core simulation was neither prepared nor refused".to_owned(),
                        ))
                    }
                })
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
                    traces: Vec::new(),
                    exit: 4,
                    document: json!({
                        "schema_version": SIMULATE_SCHEMA_VERSION, "command": "simulate",
                        "file": req.file.display().to_string(), "config": config.display().to_string(),
                        "profile": if req.compat_tlc { if matches!(req.time_limit, crate::BudgetSetting::Explicit(_)) { "tlc+overrides" } else { "tlc" } } else { "dive" },
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
        Outcome::Cancelled => (
            "resource-limit",
            None,
            Some("simulation cancelled by caller".to_owned()),
            3,
        ),
        Outcome::Internal(s) => ("internal-error", None, Some(s.clone()), 4),
    };
    // Each trace's concrete states are released once its entry is serialized,
    // and only the entry's text and validation verdict are kept (issue #1024).
    let mut result = result;
    let sampled = std::mem::take(&mut result.traces);
    let mut traces = Vec::<String>::new();
    let mut verdicts = Vec::<TraceVerdict>::new();
    let mut trace_refusal = None;
    let mut alias_failures = Vec::<String>::new();
    'documents: for (number, trace) in sampled.into_iter().enumerate() {
        if run_budget.budget.poll() != Ok(None) {
            break;
        }
        let states: Result<Vec<_>, _> = trace
            .states
            .iter()
            .map(|state| {
                if run_budget.budget.poll() != Ok(None) {
                    return Err("run budget exhausted during trace serialization".to_owned());
                }
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
        let ending = match (&result.outcome, &trace.temporal) {
            (_, Some((ending, _))) => match ending {
                dive_sim::TemporalEnding::Lasso { back_to } => Ending::Lasso { back_to: *back_to },
                dive_sim::TemporalEnding::Stuttering => Ending::Stuttering,
                dive_sim::TemporalEnding::BadPrefix(name) => Ending::BadPrefix(name.clone()),
            },
            (
                Outcome::Violation {
                    kind, name, trace, ..
                },
                None,
            ) if *trace == number => match *kind {
                "invariant" => Ending::Invariant(name.clone().unwrap_or_default()),
                "property" => Ending::Property(name.clone().unwrap_or_default()),
                _ => Ending::Deadlock,
            },
            _ => Ending::Behavior,
        };
        let mut document = TraceDoc::behavior(states)
            .map_err(|message| crate::ApiError::Rejected { message })?
            .with_ending(ending);
        // Members drawn for the existentials of a step, which the validator
        // may use to prove them (D-040).
        for (state, members) in &trace.witnesses {
            let members: Result<Vec<_>, _> = members
                .iter()
                .map(|value| {
                    dive_trace::value_json::to_json(value, model.model().evaluator.options().ctx)
                })
                .collect();
            match members {
                Ok(members) => document = document.with_witnesses(*state, members),
                Err(feature) => {
                    trace_refusal = Some(format!("simulation witness serialization: {feature}"));
                    break 'documents;
                }
            }
        }
        // A lasso's loop closes on equal states, which the validator then
        // checks whatever VIEW or SYMMETRY is declared (D-037).
        if let Some((ending, properties)) = &trace.temporal
            && !matches!(ending, dive_sim::TemporalEnding::BadPrefix(_))
        {
            document = document
                .with_properties(properties.clone())
                .with_concrete_closure();
        }
        // The step into state `k + 1` is labelled with the action the sampler
        // chose; the validator checks each label's definition on its step.
        for (step, &action) in trace.actions.iter().enumerate() {
            if let (Some(slot), Some(action)) = (
                document.labels.get_mut(step + 1),
                model.model().actions.get(action),
            ) {
                *slot = crate::core_engine::action_name(model.model(), action).map(|name| {
                    dive_trace::Label {
                        name,
                        location: Some(action.label().to_owned()),
                    }
                });
            }
        }
        // Validation replays the given states; it never reruns the simulation.
        let validated = if matches!(
            &trace.temporal,
            Some((
                dive_sim::TemporalEnding::Lasso { .. } | dive_sim::TemporalEnding::Stuttering,
                _
            ))
        ) {
            // A lasso is checked on a fresh session: every step including
            // the closing one, fairness, and the complete false-property set.
            Ok(model.validate(&document))
        } else if matches!(result.outcome, Outcome::Sampled | Outcome::Violation { .. }) {
            model.validate_sample_mode(&document, control.clone())
        } else {
            // Diagnostic error prefixes can include the actual failing row.
            // They carry no claim that every safety obligation passed there.
            model.validate_mode(&document, control.clone())
        };
        let validation = match validated {
            Ok(validation) => validation,
            Err(error) => {
                trace_refusal = Some(format!("simulation trace validation interrupted: {error}"));
                break;
            }
        };
        let mut compressed = Vec::<usize>::new();
        for (index, state) in trace.states.iter().enumerate() {
            if run_budget.budget.poll() != Ok(None) {
                break 'documents;
            }
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
        let validation = crate::trace::report_json(&validation);
        verdicts.push(TraceVerdict {
            result: validation["result"].as_str().map(str::to_owned),
            unsupported: validation["unsupported"].as_str().map(str::to_owned),
        });
        let mut entry = json!({ "id": trace.id, "document": document.to_json(), "tlc_states": tlc_states, "validation": validation });
        if let Some(Ok(alias)) = &model.model().alias {
            let poll = || run_budget.budget.poll() != Ok(None);
            match crate::simulate_alias::render(model.model(), alias, &trace.states, &poll) {
                Ok(rendered) => entry["alias"] = Value::Array(rendered),
                Err(reason) => {
                    entry["alias"] = Value::Null;
                    alias_failures.push(format!("trace {}: {reason}", trace.id));
                }
            }
        }
        traces.push(entry.to_string());
    }
    let validation_failed = verdicts.iter().any(|t| t.is("invalid"));
    let violation_validated = match &result.outcome {
        Outcome::Violation { trace, .. } => verdicts.get(*trace).is_some_and(|t| t.is("valid")),
        _ => false,
    };
    let undecidable = trace_refusal.is_some()
        || (matches!(result.outcome, Outcome::Violation { .. }) && !violation_validated)
        || (matches!(result.outcome, Outcome::Sampled) && verdicts.iter().any(|t| !t.is("valid")));
    let (reported_kind, reported_code, reported_message) = if validation_failed {
        (
            "internal-error",
            None,
            Some("independent simulation trace validation rejected the candidate".to_owned()),
        )
    } else if let Some(feature) = trace_refusal {
        ("unsupported", None, Some(feature))
    } else if undecidable {
        let reasons: Vec<_> = verdicts
            .iter()
            .filter_map(|t| t.unsupported.as_deref())
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
    let mut exit = if validation_failed {
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
        .chain(
            symmetry_warnings
                .iter()
                .map(|(code, text)| json!({"kind": "warning", "code": code, "text": text})),
        )
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
    let mut document = json!({
        "schema_version": SIMULATE_SCHEMA_VERSION, "command": "simulate",
        "file": req.file.display().to_string(), "config": config.display().to_string(),
        "profile": if req.compat_tlc { if matches!(req.time_limit, crate::BudgetSetting::Explicit(_)) { "tlc+overrides" } else { "tlc" } } else { "dive" },
        "result": reported_kind, "observed_result": kind, "observed_tlc_code": code, "exhaustive": false, "model_proved": false,
        "tlc_code": reported_code, "message": reported_message, "exit_code": exit,
        "violation_validated": violation_validated, "observed_violation": violation, "pending_violation": pending_violation, "startup_output": startup,
        "options": { "seed": seed, "seed_given": req.seed.is_some(), "aril": options.aril, "aril_requested": req.aril,
            "depth": options.depth,
            "traces": if options.traces == UNLIMITED_TRACES { Value::Null } else { json!(options.traces) },
            "keep_traces": if options.keep_traces == KEEP_ALL_TRACES { json!("all") } else { json!(options.keep_traces) },
            "max_report_bytes": req.max_report_bytes, "max_generated": req.max_generated,
            "workers_requested": workers,
            "workers_used": if result.stage == "preflight" { 0 } else { workers },
            "execution_stage": result.stage, "execution_scope": "simulation-backend",
            "mode": "finite-safety", "probabilistic": req.generation,
            "algorithm": if req.generation { "constructive" } else { "enumerating" },
            "initial_states": if req.constructive_init { "constructed" } else { "enumerated" },
            "sample_unbounded": match options.sample_unbounded {
                crate::SampleUnbounded::Off => json!("off"),
                crate::SampleUnbounded::Geometric { .. } => json!("geometric"),
            },
            "sample_mean": match options.sample_unbounded {
                crate::SampleUnbounded::Off => Value::Null,
                crate::SampleUnbounded::Geometric { mean } => json!(mean),
            },
            "action_stats": req.action_stats,
            "max_output_events": options.max_output_events, "max_output_bytes": options.max_output_bytes },
        "statistics": { "generated": result.generated, "initial_candidates": result.initial_candidates, "traces_started": result.traces_started,
            "traces_kept": traces.len(), "traces_dropped": 0,
            "traces_stalled": result.traces_stalled, "sampled_sets": result.sampled_sets,
            "scope": if matches!(result.outcome, Outcome::Sampled) { "complete-request" } else { "completed-prefix" } },
        "traces": [], "action_flow": action_flow,
        "runtime_output": result.runtime_output.iter().map(|event| json!({
            "kind": if event.warning { "warning" } else { "print" }, "text": event.text,
        })).collect::<Vec<_>>(),
    });
    document["options"]["liveness_extend"] = json!(options.liveness_extend);
    document["options"]["liveness_budget"] = options
        .liveness_budget
        .map_or_else(|| json!("auto"), |budget| json!(budget));
    document["options"]["bad_prefix"] = json!(options.bad_prefix);
    if let Some(temporal) = &result.temporal {
        let classes = &temporal.classes;
        document["statistics"]["trace_classes"] = json!({
            "closed": classes.closed, "terminal": classes.terminal, "truncated": classes.truncated,
            "extended_closed": classes.extended_closed, "extended_terminal": classes.extended_terminal,
            "violated": classes.violated, "cancelled": classes.cancelled,
        });
        document["statistics"]["properties"] = Value::Array(
            temporal
                .properties
                .iter()
                .map(|property| {
                    let unchecked: u64 = property.unchecked.values().sum();
                    let reasons: serde_json::Map<String, Value> = property
                        .unchecked
                        .iter()
                        .map(|(reason, count)| (reason.code().to_owned(), json!(count)))
                        .collect();
                    json!({
                        "name": property.name, "part": "temporal",
                        "refuted_by_trace": property.refuted_by_trace,
                        "checked_traces": property.checked, "unchecked_traces": unchecked,
                        "unchecked_reasons": reasons, "behaviors_checked": property.checked,
                    })
                })
                .collect(),
        );
    }
    // Each trace entry's `alias` renders its validated states; a trace whose
    // ALIAS could not be rendered says why here.
    let mut notes = Vec::new();
    if let Some(Ok(alias)) = &model.model().alias {
        for failure in &alias_failures {
            notes.push(format!("ALIAS {} not rendered for {failure}", alias.name));
        }
    }
    let declared: Vec<String> = [
        model
            .model()
            .symmetry
            .as_ref()
            .map(|name| format!("SYMMETRY {name}")),
        model
            .model()
            .view
            .as_ref()
            .map(|view| format!("VIEW {}", view.name)),
    ]
    .into_iter()
    .flatten()
    .collect();
    if let (crate::SampleUnbounded::Geometric { mean }, false) =
        (options.sample_unbounded, result.sampled_sets.is_empty())
    {
        notes.push(format!(
            "Unbounded sets were sampled (D-040), not enumerated: Nat geometric with mean {mean} (P(k) = p(1-p)^k, p = 1/{}), Int a geometric magnitude with a uniform sign, Seq(S) a geometric length with elements drawn from S, finite sets uniformly; seed {seed}. Sets sampled: {}. Values not drawn were not checked.",
            mean + 1,
            result.sampled_sets.join("; ")
        ));
    }
    if result.traces_stalled > 0 {
        notes.push(format!(
            "{} trace(s) ended early: no member drawn from an unbounded set led to an admitted state after {} attempts. This is not a deadlock.",
            result.traces_stalled, dive_sim::SAMPLE_ATTEMPTS
        ));
    }
    if !declared.is_empty() {
        notes.push(format!(
            "{} {} not affect simulation, which never deduplicates states.",
            declared.join(" and "),
            if declared.len() == 1 { "does" } else { "do" }
        ));
    }
    document["notes"] = json!(notes);
    document["engine"] = json!("core");
    document["engine_requested"] = json!("core");
    // Retained in the report schema; there is no automatic fallback.
    document["engine_fallback"] = Value::Null;
    document["options"]["execution_scope"] = json!("compiled-plan-simulation");
    drop(control_scope);
    run_budget.finish(
        &mut document,
        &mut exit,
        matches!(result.outcome, Outcome::Cancelled),
    )?;
    let mut report = SimulateReport {
        document,
        traces,
        exit,
    };
    if let Some(bound) = req.max_report_bytes {
        report.fit(bound);
    }
    Ok(report)
}

impl SimulateReport {
    /// The exact number of bytes [`Self::write_json`] writes with
    /// `convention`, without building the text.
    ///
    /// ```
    /// # fn show(report: &dive_api::SimulateReport) -> std::io::Result<()> {
    /// let mut text = Vec::new();
    /// report.write_json(&mut text, None)?;
    /// assert_eq!(report.json_len(None), text.len());
    /// # Ok(()) }
    /// ```
    #[must_use]
    pub fn json_len(&self, convention: Option<crate::ExitConvention>) -> usize {
        struct Count(usize);
        impl std::io::Write for Count {
            fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
                self.0 += bytes.len();
                Ok(bytes.len())
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        let mut count = Count(0);
        // Writing to a counter cannot fail.
        let _ = self.write_json(&mut count, convention);
        count.0
    }

    /// The longest of the report's renderings, under either exit convention.
    fn widest_len(&self) -> usize {
        [
            None,
            Some(crate::ExitConvention::Native),
            Some(crate::ExitConvention::Tlc),
        ]
        .into_iter()
        .map(|convention| self.json_len(convention))
        .max()
        .unwrap_or(0)
    }

    /// Drops kept sample traces from the highest id down until the report
    /// fits `bound` bytes; then, if needed, replaces the deciding trace by a
    /// summary. Samples are the lowest ids and the deciding trace the last
    /// entry, so its position only follows the drops. Its validation is kept
    /// either way.
    fn fit(&mut self, bound: usize) {
        let deciding = self.document["observed_violation"]["trace"].is_u64();
        let mut dropped = 0u64;
        while self.widest_len() > bound {
            let samples = self.traces.len() - usize::from(deciding);
            if samples == 0 {
                break;
            }
            self.traces.remove(samples - 1);
            dropped += 1;
            self.document["statistics"]["traces_dropped"] = json!(dropped);
            self.document["statistics"]["traces_kept"] = json!(self.traces.len());
            if deciding {
                self.document["observed_violation"]["trace"] = json!(self.traces.len() - 1);
            }
        }
        if deciding
            && self.widest_len() > bound
            && let Some(entry) = self.traces.last_mut()
            && let Ok(entry_json) = serde_json::from_str::<Value>(entry)
        {
            let document = entry_json["document"].to_string();
            *entry = json!({
                "id": entry_json["id"],
                "summary": {
                    "reason": "report byte bound",
                    "length": entry_json["document"]["states"].as_array().map_or(0, Vec::len),
                    "ending": entry_json["document"]["ending"],
                    "document_sha256": dive_runtime::checkpoint::hash(document.as_bytes()),
                },
                "validation": entry_json["validation"],
            })
            .to_string();
        }
    }
}

/// The parts of one trace's validation report that decide the run's result.
struct TraceVerdict {
    result: Option<String>,
    unsupported: Option<String>,
}

impl TraceVerdict {
    fn is(&self, result: &str) -> bool {
        self.result.as_deref() == Some(result)
    }
}

/// Prepares a `SYMMETRY` declaration exactly as `dive check` does at
/// startup, for its errors and warnings; simulation then uses no reduction.
fn prepare_symmetry(spec: &SpecModel, warnings: &mut Vec<(u32, String)>) -> Result<(), Outcome> {
    let model = spec.model();
    if model.symmetry.is_none() {
        return Ok(());
    }
    let prepared = model
        .prepare_symmetry(
            spec.config(),
            dive_model::symmetry::Mode::Checked,
            dive_model::symmetry::Limits::default(),
        )
        .map_err(|error| match &error.kind {
            dive_model::ErrorKind::Tlc(_) | dive_model::ErrorKind::Eval { .. } => Outcome::Error {
                code: error.code().unwrap_or(1000),
                message: error.message.clone(),
            },
            dive_model::ErrorKind::Unsupported { feature } => Outcome::Unsupported(feature.clone()),
            dive_model::ErrorKind::Bug => Outcome::Internal(error.message.clone()),
        })?;
    for message in prepared.iter().flat_map(|prepared| &prepared.warnings) {
        match message {
            dive_model::Message::Warning { code, message } => {
                warnings.push((code.code(), message.clone()));
            }
            dive_model::Message::Print(_) => {
                return Err(Outcome::Unsupported(
                    "printed output while preparing SYMMETRY".to_owned(),
                ));
            }
        }
    }
    Ok(())
}

fn tlc_exit(code: u32) -> u8 {
    match code {
        2107 | 2110 => 12,
        2108 | 2112 | 2113 | 2116 => 13,
        2114 => 11,
        2104 | 2105 => 10,
        2109 => 75,
        2111 => 76,
        _ => 255,
    }
}
