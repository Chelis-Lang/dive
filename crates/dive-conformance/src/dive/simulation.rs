//! Finite simulation admission. A complete sample is never an exhaustive proof.

use dive_api::trace::{
    Ending, RunMode, SimulationConfig, SimulationRun, SpecModel, TraceDoc, TraceOptions, Validation,
};

use super::{
    CheckMapping, Counts, DiveImplementation, ImplResult, ModelRequest, OracleResult,
    ProcessOutcome, Profile, Result, Trace, TraceEnding, Verdict, ec_name, engine_run,
    exit_status_name, unclassified,
};
use serde_json::Value;

impl DiveImplementation {
    pub(super) fn check_simulation(
        &self,
        req: &ModelRequest<'_>,
        traces: u64,
    ) -> Result<ImplResult<OracleResult>> {
        if self.symmetry_policy().is_some()
            || req
                .model
                .features
                .iter()
                .any(|feature| feature == "action composition")
        {
            return Ok(ImplResult::Unsupported(
                "finite simulation does not accept the requested BFS-specific options".to_owned(),
            ));
        }
        let config = req.corpus_root.join(&req.model.id);
        let count = traces.to_string();
        let workers = req.workers.to_string();
        let time = if req.budgets {
            format!("{}ms", req.timeout.as_millis().max(1))
        } else {
            "none".to_owned()
        };
        let memory = if req.budgets { req.xmx } else { "none" };
        let constructive = req
            .model
            .features
            .iter()
            .any(|feature| feature == "constructive generation");
        let mut extra: Vec<&std::ffi::OsStr> = vec![
            "--config".as_ref(),
            config.as_os_str(),
            "--traces".as_ref(),
            count.as_ref(),
            "--depth".as_ref(),
            "100".as_ref(),
            "--workers".as_ref(),
            workers.as_ref(),
            // Every trace is validated independently below.
            "--keep-traces".as_ref(),
            "all".as_ref(),
            "--seed".as_ref(),
            "0".as_ref(),
            "--time-limit".as_ref(),
            time.as_ref(),
            "--max-memory".as_ref(),
            memory.as_ref(),
            "--exit-codes".as_ref(),
            "tlc".as_ref(),
            "--engine".as_ref(),
            self.engine.name().as_ref(),
        ];
        if constructive {
            extra.push("--generate".as_ref());
        }
        for path in &self.module_path {
            extra.extend(["--module-path".as_ref(), path.path.as_os_str()]);
        }
        let (command, outcome) = self.run(
            "simulate",
            &extra,
            req.corpus_root,
            &req.model.module,
            if req.budgets {
                req.timeout.saturating_add(crate::pipeline::KILL_GRACE)
            } else {
                req.timeout
            },
        )?;
        let mut mapped = simulation_result(self, req, &outcome, traces, constructive);
        if matches!(&mapped, CheckMapping::Result(result) if matches!(result.verdict, Verdict::Success | Verdict::SafetyViolation | Verdict::ActionPropertyViolation | Verdict::Deadlock))
            && let Some(reason) = self.external_samples(req, &outcome)?
        {
            mapped = unclassified(outcome.exit_code, reason);
        }
        Ok(match mapped {
            CheckMapping::Refused(reason) => ImplResult::Unsupported(reason),
            CheckMapping::Result(result) => ImplResult::Completed {
                result: *result,
                command,
                elapsed: outcome.elapsed,
            },
        })
    }

    fn external_samples(
        &self,
        req: &ModelRequest<'_>,
        outcome: &ProcessOutcome,
    ) -> Result<Option<String>> {
        let value: Value = serde_json::from_str(&outcome.stdout)
            .map_err(|error| crate::error::Error::Config(error.to_string()))?;
        let scratch = crate::util::TempDir::new_in(&self.tmp_root, "sample-validation")?;
        for word in value["traces"].as_array().into_iter().flatten() {
            let document = &word["document"];
            if document["ending"]["kind"] != "behavior" {
                continue;
            }
            let path = scratch.path().join("sample.json");
            std::fs::write(&path, document.to_string())
                .map_err(|error| crate::error::Error::io(&path, &error))?;
            let config = req.corpus_root.join(&req.model.id);
            let mut extra: Vec<&std::ffi::OsStr> = vec![
                "--sample".as_ref(),
                "--trace".as_ref(),
                path.as_os_str(),
                "--config".as_ref(),
                config.as_os_str(),
                "--seed".as_ref(),
                "0".as_ref(),
            ];
            for path in &self.module_path {
                extra.extend(["--module-path".as_ref(), path.path.as_os_str()]);
            }
            let (_, checked) = self.run(
                "validate-trace",
                &extra,
                req.corpus_root,
                &req.model.module,
                req.timeout,
            )?;
            if !sample_completed(&checked) {
                return Ok(Some(
                    "fresh validate-trace --sample did not establish finite sample validity"
                        .to_owned(),
                ));
            }
        }
        Ok(None)
    }
}

fn sample_completed(checked: &ProcessOutcome) -> bool {
    let report: Value = serde_json::from_str(&checked.stdout).unwrap_or(Value::Null);
    checked.exit_code == Some(0)
        && checked.signal.is_none()
        && !checked.timed_out
        && checked.stderr.trim().is_empty()
        && crate::dive::supported_report_version(&report)
        && report["command"] == "validate-trace"
        && report["validation_goal"] == "sample"
        && report["result"] == "valid"
}

#[allow(
    clippy::too_many_lines,
    reason = "one strict finite-mode envelope and independent-word admission path"
)]
fn simulation_result(
    implementation: &DiveImplementation,
    req: &ModelRequest<'_>,
    outcome: &ProcessOutcome,
    traces: u64,
    constructive: bool,
) -> CheckMapping {
    if outcome.timed_out {
        let mut result = unclassified(
            outcome.exit_code,
            "finite simulation child timed out".to_owned(),
        );
        if let CheckMapping::Result(result) = &mut result {
            result.verdict = Verdict::Timeout;
        }
        return result;
    }
    let value: Value = match serde_json::from_str(&outcome.stdout) {
        Ok(value) => value,
        Err(error) if outcome.exit_code == Some(3) => {
            return CheckMapping::Refused(format!(
                "simulation CLI refusal: {}; {error}",
                outcome.stderr.trim()
            ));
        }
        Err(error) => {
            return unclassified(
                outcome.exit_code,
                format!("unreadable simulation report: {error}"),
            );
        }
    };
    let malformed =
        |message: &str| unclassified(outcome.exit_code, format!("simulation report: {message}"));
    if outcome.signal.is_some() || !outcome.stderr.trim().is_empty() {
        return malformed("unexpected signal or stderr contradicts a complete report");
    }
    let profile = if implementation.profile == Profile::Dive {
        "dive"
    } else if req.budgets {
        "tlc+overrides"
    } else {
        "tlc"
    };
    let options = &value["options"];
    if !crate::dive::supported_report_version(&value)
        || value["command"] != "simulate"
        || value["exhaustive"] != false
        || value["model_proved"] != false
        || value["profile"] != profile
        || options["seed"] != 0
        || options["aril"] != 0
        || options["aril_requested"] != 0
        || options["mode"] != "finite-safety"
        || options["traces"] != traces
        || options["depth"] != 100
        || options["workers_requested"] != req.workers
        || options["exit_codes"] != "tlc"
        || options["algorithm"]
            != if constructive {
                "constructive"
            } else {
                "enumerating"
            }
        || options["probabilistic"] != constructive
        || value["exit_code"].as_i64() != outcome.exit_code.map(i64::from)
        || value["engine_requested"] != implementation.engine.name()
    {
        return malformed("schema, request, profile, algorithm or physical exit mismatch");
    }
    let Some(engine) = value["engine"]
        .as_str()
        .filter(|engine| matches!(*engine, "core" | "reference"))
    else {
        return malformed("actual engine is absent");
    };
    if engine != implementation.engine.name() || !value["engine_fallback"].is_null() {
        return malformed("engine does not match explicit request");
    }
    let kind = value["result"].as_str().unwrap_or_default();
    if matches!(kind, "unsupported" | "resource-limit") && outcome.exit_code == Some(3) {
        return CheckMapping::Refused(format!("{kind}: {}", value["message"]));
    }
    if kind == "stopped" && outcome.exit_code == Some(5) {
        let mut result = unclassified(
            outcome.exit_code,
            format!("finite simulation budget stop: {}", value["message"]),
        );
        if let CheckMapping::Result(result) = &mut result {
            result.verdict = Verdict::ResourceLimit;
        }
        return result;
    }
    // The ordinary simulation producer does not yet serialize independently
    // owned runtime-error endpoints. A behavior prefix cannot certify a cause.
    if kind == "evaluation-error" {
        return CheckMapping::Refused(
            "simulation evaluation error lacks an independently proved deciding endpoint"
                .to_owned(),
        );
    }
    let initial_violation = kind == "violation" && options["execution_stage"] == "initialization";
    if !matches!(kind, "sampled" | "violation")
        || options["workers_used"] != req.workers
        || (!initial_violation && options["execution_stage"] != "sampling")
        || options["execution_scope"]
            != if engine == "core" {
                "compiled-plan-simulation"
            } else {
                "simulation-backend"
            }
        || value["observed_result"] != kind
        || options["action_stats"] != false
        || !value["action_flow"].is_null()
    {
        return malformed("no complete native sampling stage");
    }
    let statistics = &value["statistics"];
    let (Some(generated), Some(initial), Some(started)) = (
        statistics["generated"].as_u64(),
        statistics["initial_candidates"].as_u64(),
        statistics["traces_started"].as_u64(),
    ) else {
        return malformed("actual sampling statistics are absent");
    };
    let Some(words) = value["traces"].as_array() else {
        return malformed("trace array is absent");
    };
    if generated < initial
        || initial == 0
        || started > traces
        || if initial_violation {
            started != 0 || words.len() != 1
        } else {
            started == 0 || words.len() as u64 != started
        }
    {
        return malformed("inconsistent trace/work partition");
    }
    let Some(minimum) = minimum_generated(initial, words) else {
        return malformed("sampled rows cannot form a bounded generated-work partition");
    };
    if generated < minimum {
        return malformed(
            "generated work is less than Init candidates plus all sampled transitions",
        );
    }
    let (code, verdict, property, selected) = if kind == "sampled" {
        if outcome.exit_code != Some(0)
            || !value["tlc_code"].is_null()
            || !value["observed_tlc_code"].is_null()
            || started != traces
            || statistics["scope"] != "complete-request"
            || !value["pending_violation"].is_null()
            || !value["observed_violation"].is_null()
        {
            return malformed("sample completion is missing or contradictory");
        }
        (2210, Verdict::Success, None, None)
    } else {
        let claim = &value["observed_violation"];
        let Some(index) = claim["trace"]
            .as_u64()
            .and_then(|index| usize::try_from(index).ok())
        else {
            return malformed("violation trace index is absent");
        };
        let Some(code) = value["tlc_code"]
            .as_i64()
            .and_then(|code| i32::try_from(code).ok())
        else {
            return malformed("violation code is absent");
        };
        let (verdict, status, name) = match (claim["kind"].as_str(), code) {
            (Some("invariant"), 2107 | 2110) => (
                Verdict::SafetyViolation,
                12,
                claim["name"].as_str().map(str::to_owned),
            ),
            (Some("property"), 2108 | 2112) => (
                Verdict::ActionPropertyViolation,
                13,
                claim["name"].as_str().map(str::to_owned),
            ),
            (Some("deadlock"), 2114) if claim["name"].is_null() => (Verdict::Deadlock, 11, None),
            _ => return malformed("violation kind/code relationship is invalid"),
        };
        if index >= words.len()
            || index.checked_add(1) != Some(words.len())
            || outcome.exit_code != Some(status)
            || value["observed_tlc_code"] != code
            || (initial_violation != matches!(code, 2107 | 2108))
            || (verdict != Verdict::Deadlock && name.as_ref().is_none_or(String::is_empty))
            || !value["pending_violation"].is_null()
            || statistics["scope"] != "completed-prefix"
        {
            return malformed("violation identity/physical exit is inconsistent");
        }
        (code, verdict, name, Some(index))
    };
    let paths = implementation
        .module_path
        .iter()
        .map(|path| path.path.clone())
        .collect();
    // The record `TLCGet("config")` reads, from the request alone: the
    // logical worker count by default, the physical one under TLC's profile.
    let run = expected_run(
        constructive,
        traces,
        if implementation.profile == Profile::Dive {
            1
        } else {
            usize::try_from(req.workers).unwrap_or(usize::MAX)
        },
        implementation.profile != Profile::Dive,
    );
    let source = match SpecModel::load(
        &req.corpus_root.join(&req.model.module),
        &req.corpus_root.join(&req.model.id),
        &TraceOptions {
            compat_tlc: implementation.profile != Profile::Dive,
            run: Some((0, 0)),
            run_mode: RunMode::Simulate(run.simulation.clone()),
            run_workers: Some(run.workers),
            module_paths: paths,
            ..Default::default()
        },
    ) {
        Ok(source) => source,
        Err(error) => {
            return CheckMapping::Refused(format!("independent simulation binding: {error}"));
        }
    };
    let mut trace = None;
    for (index, word) in words.iter().enumerate() {
        if word["id"].as_u64() != Some(index as u64) {
            return malformed("trace identities are not the requested prefix");
        }
        let document = match TraceDoc::from_json(&word["document"]) {
            Ok(document) => document,
            Err(error) => return malformed(&format!("unreadable state word: {error}")),
        };
        if document
            .run
            .as_ref()
            .is_some_and(|recorded| *recorded != run)
        {
            return malformed("word's run settings disagree with the request");
        }
        let expected = if Some(index) == selected {
            match verdict {
                Verdict::SafetyViolation => {
                    Some(Ending::Invariant(property.clone().unwrap_or_default()))
                }
                Verdict::ActionPropertyViolation => {
                    Some(Ending::Property(property.clone().unwrap_or_default()))
                }
                Verdict::Deadlock => Some(Ending::Deadlock),
                _ => None,
            }
        } else {
            Some(Ending::Behavior)
        };
        if document.ending != expected
            || document.states.is_empty()
            || document.states.len() > 101
            || (expected == Some(Ending::Behavior) && document.states.len() != 101)
            || (initial_violation && document.states.len() != 1)
            || (Some(index) == selected && matches!(code, 2110 | 2112) && document.states.len() < 2)
        {
            return malformed("word ending or completed depth disagrees with the report");
        }
        let checked = if expected == Some(Ending::Behavior) {
            match source.try_validate_sample(&document) {
                Ok(report) => report,
                Err(error) => {
                    return CheckMapping::Refused(format!(
                        "independent sample validation interrupted: {error}"
                    ));
                }
            }
        } else {
            source.validate(&document)
        };
        match checked.validation {
            Validation::Valid => {}
            Validation::Invalid(reason) => {
                return malformed(&format!("independently rejected state word: {reason:?}"));
            }
            Validation::Unsupported(reason) => {
                return CheckMapping::Refused(format!("independent simulation word: {reason}"));
            }
            other => {
                return malformed(&format!("independent simulation word: {}", other.kind()));
            }
        }
        if Some(index) == selected {
            trace = Some(Trace {
                length: document.states.len() as u64,
                ending: TraceEnding::Finite,
                json: Some(word["document"].clone()),
                steps: None,
                omitted: None,
                json_unavailable: None,
            });
        }
    }
    CheckMapping::Result(Box::new(OracleResult {
        native_unreduced_symmetry: false,
        engine: engine_run(&value),
        verdict,
        verdict_code: Some(code),
        error_class: ec_name(code).map(str::to_owned),
        exit_code: outcome.exit_code,
        exit_status: outcome
            .exit_code
            .and_then(exit_status_name)
            .map(str::to_owned),
        violated_property: property,
        counts: Counts {
            states_generated: Some(generated),
            ..Counts::default()
        },
        fingerprint_collision: None,
        trace,
        error_codes: if code == 2210 { Vec::new() } else { vec![code] },
        warning_codes: Vec::new(),
        unresolved_modules: Vec::new(),
        note: Some(
            "finite independently replayed sample; exhaustive=false/model_proved=false (ADR 0116)"
                .to_owned(),
        ),
    }))
}

/// Each retained trace reuses an already generated initial candidate. Every
/// following row requires at least one further complete emitted candidate,
/// including a violating terminal row. Filtering and rejected candidates can
/// add work, so this is a lower bound rather than an equality with sample size.
fn minimum_generated(initial: u64, words: &[Value]) -> Option<u64> {
    words.iter().try_fold(initial, |total, word| {
        let rows = u64::try_from(word["document"]["states"].as_array()?.len()).ok()?;
        total.checked_add(rows.checked_sub(1)?)
    })
}

/// One `dive simulate` run of the simulation study
/// ([`crate::simulation_study`]): its verdict class and, for a violation,
/// the independent validation of the deciding trace.
#[derive(Debug, Clone)]
pub struct StudyRun {
    /// Verdict class, as [`Verdict::as_str`], or `unsupported` / `cli-error`.
    pub class: String,
    /// Violated property or invariant name.
    pub property: Option<String>,
    /// Independent validation of a violation's deciding trace.
    pub validation: Option<crate::trace_check::TraceCheck>,
    /// The refusal or error message, if any.
    pub message: Option<String>,
    /// Wall time.
    pub elapsed_ms: u64,
    /// Per temporal property: (name, traces checked, traces unchecked).
    pub temporal: Vec<(String, u64, u64)>,
    /// The command line, redacted.
    pub command: Vec<String>,
}

/// Parameters of one study run.
#[derive(Debug, Clone, Copy)]
pub struct StudyParams<'a> {
    /// Total traces.
    pub traces: u64,
    /// Trace depth.
    pub depth: u64,
    /// Seed.
    pub seed: i64,
    /// `--liveness-extend` value, when not the default.
    pub extend: Option<&'a str>,
    /// Child timeout; dive's own time limit is slightly shorter.
    pub timeout: std::time::Duration,
    /// `--max-memory`.
    pub memory: &'a str,
}

impl DiveImplementation {
    /// Runs `dive simulate` once for the simulation study, keeping only the
    /// deciding trace, and validates that trace with a fresh model.
    ///
    /// # Errors
    /// Fails only on harness process errors; every dive outcome is a run.
    pub fn simulation_study_run(
        &self,
        corpus_root: &std::path::Path,
        model: &crate::corpus::ModelEntry,
        params: StudyParams<'_>,
    ) -> Result<StudyRun> {
        let config = corpus_root.join(&model.id);
        let (traces, depth, seed) = (
            params.traces.to_string(),
            params.depth.to_string(),
            params.seed.to_string(),
        );
        let limit = format!(
            "{}ms",
            params.timeout.as_millis().saturating_sub(5_000).max(1_000)
        );
        let mut extra: Vec<&std::ffi::OsStr> = vec![
            "--config".as_ref(),
            config.as_os_str(),
            "--traces".as_ref(),
            traces.as_ref(),
            "--depth".as_ref(),
            depth.as_ref(),
            "--seed".as_ref(),
            seed.as_ref(),
            "--workers".as_ref(),
            "1".as_ref(),
            "--keep-traces".as_ref(),
            "0".as_ref(),
            "--time-limit".as_ref(),
            limit.as_ref(),
            "--max-memory".as_ref(),
            params.memory.as_ref(),
            "--exit-codes".as_ref(),
            "tlc".as_ref(),
            "--engine".as_ref(),
            self.engine.name().as_ref(),
        ];
        if model
            .features
            .iter()
            .any(|f| f == "constructive generation")
        {
            extra.push("--generate".as_ref());
        }
        if let Some(extend) = params.extend {
            extra.extend::<[&std::ffi::OsStr; 2]>(["--liveness-extend".as_ref(), extend.as_ref()]);
        }
        for path in &self.module_path {
            extra.extend(["--module-path".as_ref(), path.path.as_os_str()]);
        }
        let (command, outcome) = self.run(
            "simulate",
            &extra,
            corpus_root,
            &model.module,
            params.timeout,
        )?;
        let mut run = StudyRun {
            class: String::new(),
            property: None,
            validation: None,
            message: None,
            elapsed_ms: u64::try_from(outcome.elapsed.as_millis()).unwrap_or(u64::MAX),
            temporal: Vec::new(),
            command,
        };
        if outcome.timed_out {
            Verdict::Timeout.as_str().clone_into(&mut run.class);
            return Ok(run);
        }
        let Ok(value) = serde_json::from_str::<Value>(&outcome.stdout) else {
            run.class = "cli-error".into();
            run.message = outcome
                .stderr
                .lines()
                .rev()
                .find(|l| !l.trim().is_empty())
                .map(|l| l.chars().take(200).collect());
            return Ok(run);
        };
        let generate = model
            .features
            .iter()
            .any(|f| f == "constructive generation");
        let mut expected = expected_run(generate, params.traces, 1, self.profile != Profile::Dive);
        expected.simulation.depth = i64::try_from(params.depth).unwrap_or(i64::MAX);
        self.read_study_report(corpus_root, model, params.seed, &expected, &value, &mut run);
        Ok(run)
    }

    /// Fills `run` from a parsed `dive simulate` report, validating the
    /// deciding trace of a violation.
    fn read_study_report(
        &self,
        corpus_root: &std::path::Path,
        model: &crate::corpus::ModelEntry,
        seed: i64,
        expected: &SimulationRun,
        value: &Value,
        run: &mut StudyRun,
    ) {
        run.message = value["message"]
            .as_str()
            .map(|m| m.chars().take(200).collect());
        for p in value["statistics"]["properties"]
            .as_array()
            .into_iter()
            .flatten()
        {
            if p["part"] == "temporal" {
                run.temporal.push((
                    p["name"].as_str().unwrap_or_default().to_owned(),
                    p["checked_traces"].as_u64().unwrap_or(0),
                    p["unchecked_traces"].as_u64().unwrap_or(0),
                ));
            }
        }
        let code = value["tlc_code"]
            .as_i64()
            .and_then(|c| i32::try_from(c).ok());
        let verdict = match value["result"].as_str().unwrap_or_default() {
            "sampled" => Some(Verdict::Success),
            "violation" | "evaluation-error" => Some(
                code.and_then(crate::tlc::codes::verdict_for_code)
                    .unwrap_or(Verdict::EvaluationError),
            ),
            "stopped" | "resource-limit" => Some(Verdict::ResourceLimit),
            _ => None,
        };
        let Some(verdict) = verdict else {
            value["result"]
                .as_str()
                .unwrap_or("unclassified")
                .clone_into(&mut run.class);
            return;
        };
        verdict.as_str().clone_into(&mut run.class);
        if value["result"] != "violation" {
            return;
        }
        let claim = &value["observed_violation"];
        run.property = claim["name"].as_str().map(str::to_owned);
        // `observed_violation.trace` indexes the report's trace array.
        let deciding = claim["trace"]
            .as_u64()
            .and_then(|i| usize::try_from(i).ok())
            .and_then(|i| value["traces"].as_array()?.get(i));
        run.validation = Some(match deciding.map(|t| &t["document"]) {
            Some(document) if !document.is_null() => {
                self.validate_study_word(corpus_root, model, seed, expected, document)
            }
            _ => {
                crate::trace_check::TraceCheck::no("the report carries no deciding trace document")
            }
        });
    }

    fn validate_study_word(
        &self,
        corpus_root: &std::path::Path,
        model: &crate::corpus::ModelEntry,
        seed: i64,
        expected: &SimulationRun,
        document: &Value,
    ) -> crate::trace_check::TraceCheck {
        use crate::trace_check::TraceCheck;
        let doc = match TraceDoc::from_json(document) {
            Ok(doc) => doc,
            Err(e) => return TraceCheck::refuted(format!("unreadable trace document: {e}")),
        };
        if doc
            .run
            .as_ref()
            .is_some_and(|recorded| recorded != expected)
        {
            return TraceCheck::refuted("the trace's run settings disagree with the request");
        }
        let spec = match SpecModel::load(
            &corpus_root.join(&model.module),
            &corpus_root.join(&model.id),
            &TraceOptions {
                compat_tlc: self.profile != Profile::Dive,
                run: Some((seed, 0)),
                run_mode: RunMode::Simulate(expected.simulation.clone()),
                run_workers: Some(expected.workers),
                module_paths: self.module_path.iter().map(|p| p.path.clone()).collect(),
                ..TraceOptions::default()
            },
        ) {
            Ok(spec) => spec,
            Err(e) => return TraceCheck::no(format!("independent binding failed: {e}")),
        };
        // A simulation deadlock has no successor inside the constraints, so
        // it is checked as a sample; every other ending as an ordinary word.
        let report = if doc.ending == Some(Ending::Deadlock) {
            match spec.try_validate_sample(&doc) {
                Ok(report) => report,
                Err(e) => {
                    return TraceCheck::no(format!("independent validation interrupted: {e}"));
                }
            }
        } else {
            spec.validate(&doc)
        };
        match report.validation {
            Validation::Valid => TraceCheck::yes("validated against a fresh independent model"),
            Validation::Invalid(f) => {
                TraceCheck::refuted(format!("invalid state {}: {}", f.state, f.detail))
            }
            Validation::Unsupported(why) => {
                TraceCheck::no(format!("independent validation refused: {why}"))
            }
            other => TraceCheck::no(format!("independent validation returned {}", other.kind())),
        }
    }
}

/// The `TLCGet("config")` simulation record of a harness `dive simulate`
/// run at depth 100 (`--depth 100`, TLC's default), derived from the request
/// and not from dive's report. Under TLC's profile the total is narrowed to
/// TLC's 32-bit `IntValue`, as dive's driver narrows it.
fn expected_run(generate: bool, traces: u64, workers: usize, tlc: bool) -> SimulationRun {
    let total = i64::try_from(traces).unwrap_or(i64::MAX);
    SimulationRun {
        simulation: SimulationConfig {
            generate,
            #[allow(
                clippy::cast_possible_truncation,
                reason = "TLC's `(int)` cast of the total"
            )]
            traces: Some(if tlc { i64::from(total as i32) } else { total }),
            depth: 100,
            aril: 0,
        },
        workers: i64::try_from(workers).unwrap_or(i64::MAX),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::corpus::{ManifestExpectation, ModelEntry, ModelMode, Tier};
    use crate::dive::DiveEngine;
    use dive_api::{BudgetSetting, ExitConvention, SimulateRequest, simulate_file};
    use std::path::PathBuf;
    use std::time::Duration;

    #[test]
    fn external_validation_must_complete_the_requested_sample_operation() {
        let mut checked = ProcessOutcome { exit_code: Some(0), signal: None, timed_out: false,
            stdout: serde_json::json!({"schema_version":4,"command":"validate-trace","result":"valid","validation_goal":"sample"}).to_string(),
            stderr: String::new(), elapsed: Duration::ZERO };
        assert!(sample_completed(&checked));
        for report in [
            serde_json::json!({"schema_version":4,"command":"validate-trace","result":"valid"}),
            serde_json::json!({"schema_version":4,"command":"validate-trace","result":"unsupported","validation_goal":"sample"}),
            serde_json::json!({"schema_version":4,"command":"validate-trace","validation_completed":false,"validation_goal":"sample"}),
        ] {
            let mut bad = checked.clone();
            bad.stdout = report.to_string();
            assert!(!sample_completed(&bad));
        }
        checked.exit_code = Some(3);
        assert!(!sample_completed(&checked));
        checked.exit_code = Some(0);
        checked.signal = Some(9);
        assert!(!sample_completed(&checked));
    }

    #[test]
    fn generated_partition_reuses_init_and_checks_every_retained_step_without_overflow() {
        let word = |rows| serde_json::json!({"document":{"states":vec![Value::Null; rows]}});
        // Two traces reuse the same three Init candidates; their one and three
        // transitions require four further emissions, not two more Init runs.
        assert_eq!(minimum_generated(3, &[word(2), word(4)]), Some(7));
        assert_eq!(minimum_generated(3, &[word(1)]), Some(3));
        assert_eq!(minimum_generated(u64::MAX, &[word(2)]), None);
        assert_eq!(
            minimum_generated(u64::MAX - 1, &[word(1), word(2)]),
            Some(u64::MAX)
        );
        assert_eq!(minimum_generated(u64::MAX - 1, &[word(2), word(2)]), None);
        assert_eq!(minimum_generated(3, &[word(0)]), None);
        assert_eq!(
            minimum_generated(3, &[serde_json::json!({"document":{"states":false}})]),
            None
        );
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "genuine sampled/initial/successor reports share a bounded two-profile/two-engine mutation matrix"
    )]
    fn real_reports_bind_work_and_finite_options_in_both_profiles_and_engines()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let fixture = crate::util::TempDir::new("simulation-work-contract")?;
        let root = fixture.path();
        std::fs::write(
            root.join("M.tla"),
            "---- MODULE M ----\nEXTENDS Naturals\nVARIABLE x\nInit == x \\in 0..2\nNext == x>=0 /\\ x'=x+1\nInv == x>=0\nInitialBad == x=1\nNextBad == x<4\nFarBad == x<200\n====\n",
        )?;
        for case in ["Inv", "InitialBad", "NextBad", "FarBad"] {
            std::fs::write(
                root.join(format!("{case}.cfg")),
                format!("INIT Init\nNEXT Next\nINVARIANT {case}\n"),
            )?;
        }
        for selected in [DiveEngine::Core] {
            for tlc in [false, true] {
                let implementation = DiveImplementation {
                    options: Vec::new(),
                    engine: selected,
                    profile: if tlc { Profile::Tlc } else { Profile::Dive },
                    bin: PathBuf::new(),
                    module_path: Vec::new(),
                    provenance: None,
                    tmp_root: PathBuf::new(),
                    cache_root: PathBuf::new(),
                };
                for traces in [1, 2] {
                    for case in ["Inv", "InitialBad", "NextBad", "FarBad"] {
                        let model = ModelEntry {
                            id: format!("{case}.cfg"),
                            module: "M.tla".into(),
                            mode: ModelMode::Simulate { traces },
                            features: vec!["constructive generation".into()],
                            tier: Tier::Small,
                            out_of_scope: None,
                            counts_comparable: false,
                            expected: ManifestExpectation {
                                result: "success".into(),
                                runtime_seconds: Some(1),
                                distinct_states: None,
                                total_states: None,
                                state_depth: None,
                                workers: None,
                            },
                            worker_introspecting: false,
                        };
                        let req = ModelRequest {
                            corpus_root: root,
                            model: &model,
                            workers: 1,
                            timeout: Duration::from_secs(10),
                            xmx: "none",
                            budgets: false,
                        };
                        let report = simulate_file(&{
                            let mut built = SimulateRequest::default();
                            built.file = root.join("M.tla");
                            built.config = Some(root.join(&model.id));
                            built.compat_tlc = tlc;
                            built.generation = true;
                            built.seed = Some(0);
                            built.traces = traces;
                            built.depth = Some(100);
                            built.workers = Some(1);
                            built.time_limit = BudgetSetting::Unlimited;
                            built.max_memory = BudgetSetting::Unlimited;
                            built
                        })?;
                        let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
                        let admitted = |value: &Value| {
                            let outcome = ProcessOutcome {
                                exit_code: Some(i32::from(
                                    report.exit_code_for(ExitConvention::Tlc),
                                )),
                                signal: None,
                                timed_out: false,
                                stdout: value.to_string(),
                                stderr: String::new(),
                                elapsed: Duration::ZERO,
                            };
                            matches!(simulation_result(&implementation, &req, &outcome, traces, true),
                                CheckMapping::Result(ref result) if matches!(result.verdict, Verdict::Success | Verdict::SafetyViolation))
                        };
                        assert!(admitted(&value), "{tlc}/{case}/{traces}: {value}");
                        let words = value["traces"].as_array().ok_or("trace words")?;
                        let initial = value["statistics"]["initial_candidates"]
                            .as_u64()
                            .ok_or("Init count")?;
                        assert_eq!(initial, 3);
                        let minimum = minimum_generated(initial, words).ok_or("minimum work")?;
                        if case == "InitialBad" {
                            assert_eq!(value["statistics"]["traces_started"], 0);
                            assert_eq!(minimum, initial);
                            assert_eq!(words.len(), 1);
                            let mut successor_claim = value.clone();
                            successor_claim["options"]["execution_stage"] =
                                serde_json::json!("sampling");
                            successor_claim["statistics"]["traces_started"] = serde_json::json!(1);
                            successor_claim["tlc_code"] = serde_json::json!(2110);
                            successor_claim["observed_tlc_code"] = serde_json::json!(2110);
                            assert!(
                                !admitted(&successor_claim),
                                "a true initial failure cannot claim a one-row successor endpoint"
                            );
                        } else {
                            assert!(minimum > initial);
                            let mut overflow = value.clone();
                            overflow["statistics"]["initial_candidates"] =
                                serde_json::json!(u64::MAX);
                            overflow["statistics"]["generated"] = serde_json::json!(u64::MAX);
                            assert!(!admitted(&overflow));
                        }
                        if case == "NextBad" && traces == 2 {
                            let terminal = words.first().ok_or("terminal word")?;
                            let mut prefix = terminal.clone();
                            prefix["document"]["states"]
                                .as_array_mut()
                                .ok_or("states")?
                                .truncate(1);
                            prefix["document"]["ending"] = serde_json::json!({"kind":"behavior"});
                            let mut unfinished = value.clone();
                            unfinished["traces"] = serde_json::json!([prefix, terminal]);
                            unfinished["traces"][1]["id"] = serde_json::json!(1);
                            unfinished["statistics"]["traces_started"] = serde_json::json!(2);
                            unfinished["observed_violation"]["trace"] = serde_json::json!(1);
                            let minimum = minimum_generated(
                                initial,
                                unfinished["traces"].as_array().ok_or("words")?,
                            )
                            .ok_or("work")?;
                            unfinished["statistics"]["generated"] = serde_json::json!(minimum);
                            assert!(
                                !admitted(&unfinished),
                                "an unfinished safe prefix cannot precede the deciding trace"
                            );
                            let mut continued = unfinished.clone();
                            continued["traces"]
                                .as_array_mut()
                                .ok_or("words")?
                                .swap(0, 1);
                            continued["traces"][0]["id"] = serde_json::json!(0);
                            continued["traces"][1]["id"] = serde_json::json!(1);
                            continued["observed_violation"]["trace"] = serde_json::json!(0);
                            assert!(
                                !admitted(&continued),
                                "no trace can follow the deciding violation"
                            );
                        }
                        if case == "FarBad" && traces == 1 {
                            // This actual producer word is independently valid,
                            // but its late violation cannot satisfy depth 100.
                            let long = simulate_file(&{
                                let mut built = SimulateRequest::default();
                                built.file = root.join("M.tla");
                                built.config = Some(root.join(&model.id));
                                built.compat_tlc = tlc;
                                built.generation = true;
                                built.seed = Some(0);
                                built.traces = 1;
                                built.depth = Some(300);
                                built.workers = Some(1);
                                built.time_limit = BudgetSetting::Unlimited;
                                built.max_memory = BudgetSetting::Unlimited;
                                built
                            })?;
                            let mut late = long.to_json_with_exit_convention(ExitConvention::Tlc);
                            assert_eq!(late["result"], "violation");
                            assert!(
                                late["traces"][0]["document"]["states"]
                                    .as_array()
                                    .ok_or("states")?
                                    .len()
                                    > 101
                            );
                            late["options"]["depth"] = serde_json::json!(100);
                            let outcome = ProcessOutcome {
                                exit_code: Some(12),
                                signal: None,
                                timed_out: false,
                                stdout: late.to_string(),
                                stderr: String::new(),
                                elapsed: Duration::ZERO,
                            };
                            assert!(
                                !matches!(simulation_result(&implementation, &req, &outcome, traces, true),
                                CheckMapping::Result(ref result) if result.verdict == Verdict::SafetyViolation)
                            );
                        }
                        let mut too_little = value.clone();
                        too_little["statistics"]["generated"] = serde_json::json!(minimum - 1);
                        assert!(!admitted(&too_little));
                        let mut overhead = value.clone();
                        overhead["statistics"]["generated"] = serde_json::json!(minimum + 17);
                        assert!(
                            admitted(&overhead),
                            "legitimate discarded-candidate overhead is allowed"
                        );
                        if words.len() > 1 {
                            let mut only_first = value.clone();
                            only_first["statistics"]["generated"] = serde_json::json!(
                                minimum_generated(initial, &words[..1])
                                    .ok_or("first trace work")?
                            );
                            assert!(
                                !admitted(&only_first),
                                "work must include every retained trace"
                            );
                        }
                        for field in ["aril", "aril_requested", "mode"] {
                            for replacement in [
                                Value::Null,
                                serde_json::json!(812),
                                serde_json::json!("812"),
                                serde_json::json!(false),
                                serde_json::json!("exhaustive-proof"),
                            ] {
                                let mut forged = value.clone();
                                forged["options"][field] = replacement;
                                assert!(!admitted(&forged), "accepted {field}: {forged}");
                            }
                            let mut missing = value.clone();
                            missing["options"]
                                .as_object_mut()
                                .ok_or("options object")?
                                .remove(field);
                            assert!(!admitted(&missing), "accepted absent {field}");
                        }
                    }
                }
            }
        }
        Ok(())
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "real two-engine/profile reports and fail-closed mutations share one scoped setup"
    )]
    fn complete_real_samples_and_violations_require_exact_envelope_and_fresh_words()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root =
            PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../dive-api/tests/fixtures/generation");
        for selected in [DiveEngine::Core] {
            for tlc in [false, true] {
                let implementation = DiveImplementation {
                    options: Vec::new(),
                    engine: selected,
                    profile: if tlc { Profile::Tlc } else { Profile::Dive },
                    bin: PathBuf::new(),
                    module_path: Vec::new(),
                    provenance: None,
                    tmp_root: PathBuf::new(),
                    cache_root: PathBuf::new(),
                };
                for case in ["Wide", "Violation"] {
                    let model = ModelEntry {
                        id: format!("{case}.cfg"),
                        module: "Generation.tla".to_owned(),
                        mode: ModelMode::Simulate { traces: 1 },
                        features: vec!["constructive generation".to_owned()],
                        tier: Tier::Small,
                        out_of_scope: None,
                        counts_comparable: false,
                        expected: ManifestExpectation {
                            result: "success".to_owned(),
                            runtime_seconds: Some(1),
                            distinct_states: None,
                            total_states: None,
                            state_depth: None,
                            workers: None,
                        },
                        worker_introspecting: false,
                    };
                    let req = ModelRequest {
                        corpus_root: &root,
                        model: &model,
                        workers: 1,
                        timeout: Duration::from_secs(10),
                        xmx: "none",
                        budgets: false,
                    };
                    let report = simulate_file(&{
                        let mut built = SimulateRequest::default();
                        built.file = root.join("Generation.tla");
                        built.config = Some(root.join(&model.id));
                        built.compat_tlc = tlc;
                        built.generation = true;
                        built.seed = Some(0);
                        built.traces = 1;
                        built.depth = Some(100);
                        built.workers = Some(1);
                        built.time_limit = BudgetSetting::Unlimited;
                        built.max_memory = BudgetSetting::Unlimited;
                        built
                    })?;
                    let value = report.to_json_with_exit_convention(ExitConvention::Tlc);
                    let outcome = |value: &Value| ProcessOutcome {
                        exit_code: Some(i32::from(report.exit_code_for(ExitConvention::Tlc))),
                        signal: None,
                        timed_out: false,
                        stdout: value.to_string(),
                        stderr: String::new(),
                        elapsed: Duration::ZERO,
                    };
                    let mapped =
                        simulation_result(&implementation, &req, &outcome(&value), 1, true);
                    assert!(
                        matches!(mapped, CheckMapping::Result(ref r) if matches!(r.verdict, Verdict::Success | Verdict::SafetyViolation)),
                        "{mapped:?}"
                    );
                    let mut unauthoritative = value.clone();
                    unauthoritative["traces"][0]["validation"] =
                        serde_json::json!({"result":"forged"});
                    unauthoritative["violation_validated"] = serde_json::json!(false);
                    assert!(
                        matches!(simulation_result(&implementation, &req, &outcome(&unauthoritative), 1, true),
                        CheckMapping::Result(ref result) if matches!(result.verdict, Verdict::Success | Verdict::SafetyViolation))
                    );
                    for (pointer, replacement) in [
                        ("/schema_version", serde_json::json!(99)),
                        ("/options/algorithm", serde_json::json!("enumerating")),
                        ("/model_proved", serde_json::json!(true)),
                        ("/statistics/traces_started", serde_json::json!(0)),
                        ("/exit_code", serde_json::json!(77)),
                        (
                            "/traces/0/document/states/1/values/x",
                            serde_json::json!(-1),
                        ),
                    ] {
                        let mut forged = value.clone();
                        *forged
                            .pointer_mut(pointer)
                            .ok_or("missing mutation pointer")? = replacement;
                        assert!(
                            !matches!(simulation_result(&implementation, &req, &outcome(&forged), 1, true),
                            CheckMapping::Result(ref r) if matches!(r.verdict, Verdict::Success | Verdict::SafetyViolation)),
                            "accepted {pointer}"
                        );
                    }
                }
            }
        }
        Ok(())
    }
}
