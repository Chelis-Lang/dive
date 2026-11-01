//! Independent checking of two translated modules against the pinned oracle.
//!
//! The oracle module and native module may differ, but use identical configuration
//! bytes. This is the model-equivalence check used by the PlusCal corpus runner.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::Duration;

use dive_api::trace::{SpecModel, TraceOptions};
use serde_json::{Value, json};

use crate::cache::Cache;
use crate::compare::{CompareContext, FieldDiff, Outcome, diff_results};
use crate::config::Config;
use crate::dive::{CheckMapping, check_result, check_result_with_model, module_search_path};
use crate::divergence::{Profile, Registry};
use crate::error::{Error, Result};
use crate::implementation::classify_with_trace_pass;
use crate::oracle::{CheckMode, ClasspathEntry, Java, OracleEnv, TlcJob};
use crate::process::{ProcessOutcome, run_with_timeout};
use crate::tlc::result::{OracleResult, Verdict};
use crate::trace_check::{TraceCheck, validate_counterexample};

/// Inputs to a one-worker, seed-zero comparison in both semantic profiles.
pub struct ModelPair<'a> {
    /// Module produced by the reference translator.
    pub oracle_module: &'a Path,
    /// Module produced by the native translator.
    pub native_module: &'a Path,
    /// Configuration applied identically to both modules.
    pub configuration: &'a Path,
    /// Original corpus-relative model identity for exact registered divergences.
    pub model_id: &'a str,
    /// Additional original source directories, before pinned module libraries.
    pub module_paths: &'a [PathBuf],
    /// Native executable under test.
    pub dive: &'a Path,
    /// Per-child deadline, including the separate oracle trace pass.
    pub timeout: Duration,
    /// Whole comparison deadline, including source binding and witness replay.
    pub overall_timeout: Duration,
    /// Fresh owned directory retaining raw child output and receipts.
    pub raw_directory: &'a Path,
}

/// Runs the pinned TLC on the oracle translation and dive on the native
/// translation in both profiles. Uses the ordinary harness classifier,
/// comparison contract and fresh independent native counterexample replay.
///
/// # Errors
/// Invalid inputs, pin/setup failures or inability to supervise a child.
pub fn compare(config: &Config, cache: &Cache, pair: &ModelPair<'_>) -> Result<Value> {
    std::fs::create_dir(pair.raw_directory).map_err(|e| Error::io(pair.raw_directory, &e))?;
    let _deadline = comparison_deadline(pair.overall_timeout, pair.raw_directory)?;
    let oracle = config.oracles.default_oracle()?;
    let registry = Registry::load(&config.repo_root)?;
    let java = Java::resolve(config, cache, None)?;
    let mut env = OracleEnv::prepare(config, cache, &oracle.id, &java)?;
    let mut paths = pair.module_paths.to_vec();
    paths.extend(
        module_search_path(config, cache, &oracle.id)?
            .into_iter()
            .map(|e| e.path),
    );
    // Source directories are selected inputs, not separately pinned libraries.
    for (i, path) in pair.module_paths.iter().enumerate() {
        env.libraries.insert(
            i,
            ClasspathEntry {
                id: format!("model-pair-source-{i}"),
                pin: "selected source context".into(),
                path: path.clone(),
            },
        );
    }
    env.raw_invocations = Some(pair.raw_directory.join("oracle"));
    let oracle_root = pair
        .oracle_module
        .parent()
        .ok_or_else(|| Error::Usage("oracle module has no parent".into()))?;
    let oracle_name = filename(pair.oracle_module)?;
    let cfg_name = filename(pair.configuration)?;
    let oracle_cfg = oracle_root.join(cfg_name);
    if std::fs::read(&oracle_cfg).map_err(|e| Error::io(&oracle_cfg, &e))?
        != std::fs::read(pair.configuration).map_err(|e| Error::io(pair.configuration, &e))?
    {
        return Err(Error::Usage(
            "oracle and native configuration bytes differ".into(),
        ));
    }
    let job = TlcJob {
        corpus_root: oracle_root,
        module: oracle_name,
        config: cfg_name,
        mode: CheckMode::Exhaustive,
        features: &[],
        workers: 1,
        timeout: pair.timeout,
        xmx: "3g",
        seed: 0,
        lncheck: "final",
        fp: 0,
        dump_trace: false,
    };
    let clean = env.run_tlc(&job)?;
    let (golden, _) = classify_with_trace_pass(&env, &job, &clean)?;
    let mut profiles = serde_json::Map::new();
    for (name, compat) in [("dive", false), ("tlc", true)] {
        let record = compare_profile(pair, &paths, &golden, &registry, compat, name)?;
        profiles.insert(name.into(), record);
    }
    Ok(
        json!({"schema_version": 1, "kind": "independent-translated-models", "workers": 1,
        "seed": 0, "fp": 0, "model_id": pair.model_id, "oracle": oracle.id, "oracle_result": golden,
        "native_sha256": crate::hash::sha256_file(pair.dive)?,
        "configuration_sha256": crate::hash::sha256_file(pair.configuration)?,
        "oracle_module_sha256": crate::hash::sha256_file(pair.oracle_module)?,
        "native_module_sha256": crate::hash::sha256_file(pair.native_module)?,
        "profiles": profiles}),
    )
}

struct Deadline(std::sync::mpsc::Sender<()>);

impl Drop for Deadline {
    fn drop(&mut self) {
        let _ = self.0.send(());
    }
}

fn comparison_deadline(limit: Duration, raw: &Path) -> Result<Deadline> {
    let (send, receive) = std::sync::mpsc::channel();
    let receipt = raw.join("deadline.json");
    std::thread::Builder::new().name("translated-model-deadline".into()).spawn(move || {
        if receive.recv_timeout(limit) == Err(std::sync::mpsc::RecvTimeoutError::Timeout) {
            let _ = write_json(&receipt, &json!({"timed_out":true,"limit_ms":limit.as_millis()}));
            eprintln!("translated-model comparison reached its overall deadline; draining owned children");
            crate::process::kill_all_children();
            std::process::exit(5);
        }
    }).map_err(|e| Error::Command(format!("cannot start comparison deadline: {e}")))?;
    Ok(Deadline(send))
}

fn compare_profile(
    pair: &ModelPair<'_>,
    paths: &[PathBuf],
    golden: &OracleResult,
    registry: &Registry,
    compat: bool,
    name: &str,
) -> Result<Value> {
    let raw = pair.raw_directory.join(name);
    std::fs::create_dir(&raw).map_err(|e| Error::io(&raw, &e))?;
    let mut command = native_command(pair, paths, compat);
    let shown: Vec<_> = std::iter::once(command.get_program())
        .chain(command.get_args())
        .map(|s| s.to_string_lossy().into_owned())
        .collect();
    let run = run_with_timeout(&mut command, pair.timeout, &raw)?;
    let receipt = json!({"command": shown, "exit": run.exit_code, "signal": run.signal,
        "timed_out": run.timed_out, "elapsed_ms": run.elapsed.as_millis()});
    write_json(&raw.join("process.json"), &receipt)?;
    let document: Value = serde_json::from_str(&run.stdout).unwrap_or(Value::Null);
    if !valid_native_envelope(&document, &run) {
        return Ok(
            json!({"outcome":"error", "reason":"invalid native check report envelope",
            "exit":run.exit_code, "signal":run.signal,
            "diffs":[{"field":"native.envelope","expected":true,"actual":false}]}),
        );
    }
    let spec = SpecModel::load(
        pair.native_module,
        pair.configuration,
        &TraceOptions {
            compat_tlc: compat,
            run: Some((0, 0)),
            module_paths: paths.to_vec(),
            ..TraceOptions::default()
        },
    );
    let mapping = match &spec {
        Ok(spec) => check_result_with_model(&run, compat, spec),
        Err(_) => check_result(&run, compat),
    };
    let profile = if compat { Profile::Tlc } else { Profile::Dive };
    Ok(compare_native(
        golden,
        mapping,
        spec.as_ref().ok(),
        &run,
        registry,
        pair.model_id,
        &profile,
    ))
}

fn valid_native_envelope(report: &Value, run: &ProcessOutcome) -> bool {
    // A timeout, stop, refusal or bug remains uncredited in canonical mapping.
    // Ordinary facts must pass the same bounded envelope guard as the original
    // corpus runner, plus current explicit caller-selected exit metadata.
    if run.timed_out
        || matches!(
            report["verdict"].as_str(),
            Some("unsupported" | "resource-limit" | "internal-error" | "stopped")
        )
    {
        return true;
    }
    crate::native_envelope::valid_check_report(run.exit_code, report)
        && run.signal.is_none()
        && report["exit_code"].as_i64() == run.exit_code.map(i64::from)
        && report["options"]["exit_codes"] == "tlc"
        && report["tlc"]["exit_status"].as_i64() == run.exit_code.map(i64::from)
        && report.get("property").is_some()
}

fn filename(path: &Path) -> Result<&str> {
    path.file_name()
        .and_then(|s| s.to_str())
        .ok_or_else(|| Error::Usage(format!("invalid input filename {}", path.display())))
}

fn write_json(path: &Path, value: &Value) -> Result<()> {
    let text = serde_json::to_vec_pretty(value).map_err(|e| Error::Config(e.to_string()))?;
    std::fs::write(path, text).map_err(|e| Error::io(path, &e))
}

fn native_command(pair: &ModelPair<'_>, paths: &[PathBuf], compat: bool) -> Command {
    let mut command = Command::new(pair.dive);
    command
        .env_remove("DIVE_COMPAT")
        .env_remove("DIVE_ENGINE")
        .arg("check")
        .arg(pair.native_module)
        .arg("--config")
        .arg(pair.configuration)
        .args([
            "--workers",
            "1",
            "--seed",
            "0",
            "--fp",
            "0",
            "--time-limit",
            "none",
            "--max-memory",
            "none",
            "--exit-codes",
            "tlc",
            "--output",
            "json",
        ]);
    if compat {
        command.arg("--compat=tlc");
    }
    for path in paths {
        command.arg("--module-path").arg(path);
    }
    command
}

fn requires_word(result: &OracleResult, golden: &OracleResult) -> bool {
    result.trace.is_some()
        || matches!(
            result.verdict,
            Verdict::SafetyViolation
                | Verdict::ActionPropertyViolation
                | Verdict::LivenessViolation
                | Verdict::Deadlock
        )
        || (result.verdict == golden.verdict && golden.trace.is_some())
}

fn compare_native(
    golden: &OracleResult,
    mapping: CheckMapping,
    spec: Option<&SpecModel>,
    run: &ProcessOutcome,
    registry: &Registry,
    model_id: &str,
    profile: &Profile,
) -> Value {
    let result = match mapping {
        CheckMapping::Result(result) => result,
        CheckMapping::Refused(reason) => {
            return json!({"outcome": "unsupported", "reason": reason, "exit": run.exit_code});
        }
    };
    let mut diffs = diff_results(
        golden,
        &result,
        CompareContext {
            workers: 1,
            counts_comparable: true,
        },
    );
    let proof = requires_word(&result, golden).then(|| match spec {
        Some(spec) => validate_counterexample(spec, &result, golden),
        None => TraceCheck::no("independent validator could not bind the native translation"),
    });
    if let Some(proof) = &proof {
        crate::compare::apply_word_attribution(&mut diffs, golden, &result, proof);
        if !proof.validated {
            diffs.push(FieldDiff {
                field: if proof.refuted {
                    "trace.invalid"
                } else {
                    "trace.validated"
                }
                .into(),
                expected: json!(true),
                actual: json!(false),
            });
        }
    }
    let mut outcome = if run.timed_out || result.verdict == Verdict::Timeout {
        Outcome::Timeout
    } else if !golden.verdict.is_oracle_fact() {
        Outcome::OracleError
    } else if !diffs.is_empty() && diffs.iter().all(|diff| diff.field == "trace.validated") {
        Outcome::AgreePendingTraceValidation
    } else if !diffs.is_empty() {
        Outcome::Disagree
    } else {
        Outcome::Agree
    };
    let divergence = (outcome == Outcome::Disagree)
        .then(|| registry.excuse_observed(Some(profile), &[], model_id, golden, &result, &diffs))
        .flatten();
    if divergence.is_some() {
        outcome = Outcome::ExpectedDivergence;
    }
    json!({"outcome": outcome, "result": result, "diffs": diffs,
        "expected_divergence": divergence.map(|d| d.id.as_str()),
        "trace": proof.map(|p| json!({"validated": p.validated, "refuted": p.refuted, "detail": p.detail}))})
}

#[cfg(test)]
mod tests {
    use super::*;
    use dive_api::trace::TraceDoc;

    fn actual_outcome(report: &dive_api::CheckReport) -> ProcessOutcome {
        ProcessOutcome {
            exit_code: Some(i32::from(
                report.exit_code_for(dive_api::ExitConvention::Tlc),
            )),
            signal: None,
            timed_out: false,
            stdout: report
                .to_json_with_exit_convention(dive_api::ExitConvention::Tlc)
                .to_string(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
        }
    }

    #[test]
    fn actual_temporal_truth_and_planning_errors_keep_their_real_omitted_field()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let directory = crate::util::TempDir::new("temporal-envelope-controls")?;
        let file = directory.path().join("Temporal.tla");
        std::fs::write(
            file.with_extension("cfg"),
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        )?;
        for property in ["[]<>(x<=2)", "<>(x=3)", "<>1"] {
            std::fs::write(
                &file,
                format!(
                    "---- MODULE Temporal ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nP == {property}\n====\n"
                ),
            )?;
            for compat in [false, true] {
                let report = dive_api::check_file(&dive_api::CheckRequest {
                    file: file.clone(),
                    compat_tlc: compat,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..dive_api::CheckRequest::default()
                })?;
                let value = report.to_json_with_exit_convention(dive_api::ExitConvention::Tlc);
                let run = actual_outcome(&report);
                assert_eq!(value["check_kind"], "temporal");
                assert!(value["tlc"].get("reported").is_none());
                assert!(
                    valid_native_envelope(&value, &run),
                    "{compat}/{property}: {value}"
                );
                let CheckMapping::Result(result) = check_result(&run, compat) else {
                    return Err("real temporal report refused".into());
                };
                assert!(result.verdict.is_oracle_fact());
                for supplied in [json!(1000), json!("banana"), Value::Null] {
                    let mut forged = value.clone();
                    forged["tlc"]["reported"] = supplied;
                    assert!(!valid_native_envelope(&forged, &run), "{forged}");
                }
                let mut consistent = value.clone();
                consistent["tlc"]["reported"] = value["tlc"]["code"].clone();
                assert!(valid_native_envelope(&consistent, &run));
                if property == "<>1" {
                    consistent["verdict"] = json!("model-error");
                    assert!(!valid_native_envelope(&consistent, &run));
                }
                let mut wrong_kind = value.clone();
                wrong_kind["check_kind"] = json!("safety");
                assert!(!valid_native_envelope(&wrong_kind, &run));
                if property == "<>(x=3)" {
                    temporal_word_controls(&file, compat, &value, &run, &result)?;
                }
            }
        }
        Ok(())
    }

    fn temporal_word_controls(
        file: &Path,
        compat: bool,
        value: &Value,
        run: &ProcessOutcome,
        golden: &OracleResult,
    ) -> std::result::Result<(), Box<dyn std::error::Error>> {
        let spec = SpecModel::load(
            file,
            &file.with_extension("cfg"),
            &TraceOptions {
                compat_tlc: compat,
                run: Some((0, 0)),
                ..TraceOptions::default()
            },
        )?;
        let profile = if compat { Profile::Tlc } else { Profile::Dive };
        let compare = |value: &Value| {
            let mut changed = run.clone();
            changed.stdout = value.to_string();
            compare_native(
                golden,
                check_result(&changed, compat),
                Some(&spec),
                &changed,
                &Registry::default(),
                "Temporal.cfg",
                &profile,
            )
        };
        assert_eq!(compare(value)["outcome"], "agree");
        let mut forged = value.clone();
        let mut doc = TraceDoc::from_json(&forged["counterexample"]["trace"])?;
        doc.states[0].insert("x".into(), json!(99));
        forged["counterexample"]["trace"] = doc.to_json();
        forged["counterexample"]["states"][0]["values"]["x"] = json!(99);
        assert_ne!(compare(&forged)["outcome"], "agree");
        let mut absent = value.clone();
        absent["counterexample"] = Value::Null;
        assert_ne!(compare(&absent)["outcome"], "agree");
        let mut claims = value.clone();
        claims["property"] = json!("Invented");
        claims["properties"] = json!(["Invented"]);
        claims["counterexample"]["trace"]["properties"] = json!(["Invented"]);
        assert_ne!(compare(&claims)["outcome"], "agree");
        Ok(())
    }

    #[test]
    fn actual_temporal_startup_and_finite_endpoint_errors_still_require_reported()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
        for (corpus, name) in [
            ("temporal-demand-primary-probes", "UnusedDemandTautology"),
            ("liveness-atom-error-timing", "DelayedSiblingStateError"),
        ] {
            let file = root.join(format!(
                "conformance/corpus/{corpus}/specifications/{name}/{name}.tla"
            ));
            for compat in [false, true] {
                let report = dive_api::check_file(&dive_api::CheckRequest {
                    file: file.clone(),
                    compat_tlc: compat,
                    workers: Some(1),
                    seed: Some(0),
                    fp: Some(0),
                    ..dive_api::CheckRequest::default()
                })?;
                let value = report.to_json_with_exit_convention(dive_api::ExitConvention::Tlc);
                let run = actual_outcome(&report);
                assert_eq!(value["check_kind"], "temporal");
                assert!(value["tlc"].get("reported").is_some());
                assert!(
                    valid_native_envelope(&value, &run),
                    "{name}/{compat}: {value}"
                );
                let mut wrong_kind = value.clone();
                wrong_kind["verdict"] = json!("model-error");
                assert!(!valid_native_envelope(&wrong_kind, &run), "{name}/{compat}");
                let mut absent = value.clone();
                absent["tlc"]
                    .as_object_mut()
                    .ok_or("class is not an object")?
                    .remove("reported");
                assert!(
                    !valid_native_envelope(&absent, &run),
                    "{name}/{compat}: {absent}"
                );
            }
        }
        Ok(())
    }

    fn envelope_mutations() -> Vec<(&'static str, Value)> {
        vec![
            ("/tlc/name", json!("INVENTED_CLASS")),
            ("/tlc/name", json!("TLC_SUCCESS")),
            ("/verdict", json!("success")),
            ("/verdict", json!("banana")),
            ("/verdict", json!(null)),
            ("/tlc/reported", json!(1000)),
            ("/tlc/reported", json!("banana")),
            ("/tlc/reported", json!(null)),
            ("/tlc/code", json!(9999)),
            ("/tlc/code", json!("2110")),
            ("/tlc/code", json!(null)),
            ("/tlc/verified", json!(false)),
            ("/tlc/verified", json!("true")),
            ("/tlc/exit_status", json!(0)),
            ("/tlc/exit_status", json!("12")),
            ("/tlc/exit_status", json!(null)),
            ("/exit_code", json!(0)),
            ("/exit_code", json!("12")),
            ("/exit_code", json!(null)),
            ("/options/exit_codes", json!("native")),
            ("/options/exit_codes", json!("banana")),
            ("/statistics/distinct", json!(-1)),
            ("/statistics/distinct", json!("3")),
            ("/statistics/generated", json!(1)),
            ("/statistics/generated", json!(null)),
            ("/statistics/depth", json!(-1)),
            ("/statistics/depth", json!(null)),
            ("/property", json!("")),
            ("/property", json!(null)),
            ("/property", json!(7)),
        ]
    }

    #[test]
    fn actual_native_reports_require_the_complete_envelope_before_mapping()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let directory = crate::util::TempDir::new("native-envelope-controls")?;
        let file = directory.path().join("Envelope.tla");
        std::fs::write(
            &file,
            "---- MODULE Envelope ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nInv == x<2\n====\n",
        )?;
        std::fs::write(
            file.with_extension("cfg"),
            "INIT Init\nNEXT Next\nINVARIANT Inv\n",
        )?;
        for compat in [false, true] {
            let report = dive_api::check_file(&dive_api::CheckRequest {
                file: file.clone(),
                compat_tlc: compat,
                workers: Some(1),
                seed: Some(0),
                fp: Some(0),
                ..dive_api::CheckRequest::default()
            })?;
            let value = report.to_json_with_exit_convention(dive_api::ExitConvention::Tlc);
            let mut run = ProcessOutcome {
                exit_code: Some(12),
                signal: None,
                timed_out: false,
                stdout: value.to_string(),
                stderr: String::new(),
                elapsed: Duration::ZERO,
            };
            assert!(valid_native_envelope(&value, &run));
            for (pointer, replacement) in envelope_mutations() {
                let mut forged = value.clone();
                *forged
                    .pointer_mut(pointer)
                    .ok_or("actual report omitted required field")? = replacement;
                assert!(
                    !valid_native_envelope(&forged, &run),
                    "{compat}: {pointer}: {forged}"
                );
            }
            for field in ["statistics", "property", "exit_code", "tlc"] {
                let mut absent = value.clone();
                absent
                    .as_object_mut()
                    .ok_or("actual report is not an object")?
                    .remove(field);
                assert!(!valid_native_envelope(&absent, &run), "{field}");
            }
            for exit in [None, Some(0), Some(10), Some(255)] {
                run.exit_code = exit;
                assert!(!valid_native_envelope(&value, &run), "physical {exit:?}");
            }
            run.exit_code = Some(12);
            run.signal = Some(9);
            assert!(!valid_native_envelope(&value, &run));
        }
        Ok(())
    }

    #[test]
    fn actual_words_validate_and_forged_or_missing_words_never_agree()
    -> std::result::Result<(), Box<dyn std::error::Error>> {
        let directory = crate::util::TempDir::new("translated-pair-controls")?;
        let file = directory.path().join("Pair.tla");
        let cfg = file.with_extension("cfg");
        std::fs::write(
            &file,
            "---- MODULE Pair ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=IF x<2 THEN x+1 ELSE x\nInv == x<2\n====\n",
        )?;
        std::fs::write(&cfg, "INIT Init\nNEXT Next\nINVARIANT Inv\n")?;
        for (compat, profile) in [(false, Profile::Dive), (true, Profile::Tlc)] {
            let report = dive_api::check_file(&dive_api::CheckRequest {
                file: file.clone(),
                compat_tlc: compat,
                seed: Some(0),
                fp: Some(0),
                workers: Some(1),
                ..dive_api::CheckRequest::default()
            })?;
            let run = ProcessOutcome {
                exit_code: Some(i32::from(
                    report.exit_code_for(dive_api::ExitConvention::Tlc),
                )),
                signal: None,
                timed_out: false,
                stdout: report
                    .to_json_with_exit_convention(dive_api::ExitConvention::Tlc)
                    .to_string(),
                stderr: String::new(),
                elapsed: Duration::ZERO,
            };
            let CheckMapping::Result(golden) = check_result(&run, compat) else {
                return Err("actual producer was not classified".into());
            };
            let spec = SpecModel::load(
                &file,
                &cfg,
                &TraceOptions {
                    compat_tlc: compat,
                    run: Some((0, 0)),
                    ..TraceOptions::default()
                },
            )?;
            let compare = |result: OracleResult| {
                compare_native(
                    &golden,
                    CheckMapping::Result(Box::new(result)),
                    Some(&spec),
                    &run,
                    &Registry::default(),
                    "Pair.cfg",
                    &profile,
                )
            };
            assert_eq!(compare((*golden).clone())["outcome"], "agree");
            let mut forged = (*golden).clone();
            let Some(trace) = &mut forged.trace else {
                return Err("actual producer omitted its word".into());
            };
            let mut doc = TraceDoc::from_json(trace.json.as_ref().ok_or("no actual native JSON")?)?;
            doc.states[0].insert("x".into(), json!(99));
            trace.json = Some(doc.to_json());
            assert_eq!(compare(forged)["outcome"], "disagree");
            let mut absent = (*golden).clone();
            absent.trace = None;
            assert_ne!(compare(absent)["outcome"], "agree");
            let mut count = (*golden).clone();
            count.counts.states_generated = Some(999);
            assert_eq!(compare(count)["outcome"], "disagree");
            let refusal = compare_native(
                &golden,
                CheckMapping::Refused("not implemented".into()),
                Some(&spec),
                &run,
                &Registry::default(),
                "Pair.cfg",
                &profile,
            );
            assert_eq!(refusal["outcome"], "unsupported");
            let mut no_oracle = (*golden).clone();
            no_oracle.verdict = Verdict::Unclassified;
            assert_eq!(
                compare_native(
                    &no_oracle,
                    CheckMapping::Result(golden.clone()),
                    Some(&spec),
                    &run,
                    &Registry::default(),
                    "Pair.cfg",
                    &profile
                )["outcome"],
                "oracle-error"
            );
        }
        Ok(())
    }
}
