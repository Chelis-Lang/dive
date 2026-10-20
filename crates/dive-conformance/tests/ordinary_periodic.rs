//! Actual default producers keep prefix truth distinct from exhaustive checking.
use std::path::{Path, PathBuf};
use std::time::Duration;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions};
use dive_api::{CheckRequest, ExitConvention};
use dive_conformance::dive::{CheckMapping, check_result};
use dive_conformance::process::ProcessOutcome;
use dive_conformance::tlc::result::{Counts, OracleResult, Verdict};
use dive_conformance::trace_check::validate_counterexample;
use serde_json::{Value, json};

type TestResult = Result<(), Box<dyn std::error::Error>>;
struct Spec {
    _directory: dive_conformance::util::TempDir,
    file: PathBuf,
}
impl Spec {
    fn new(property: &str, next: &str) -> Result<Self, Box<dyn std::error::Error>> {
        let directory = dive_conformance::util::TempDir::new("ordinary-periodic")?;
        let file = directory.path().join("Ordinary.tla");
        std::fs::write(
            &file,
            format!(
                "---- MODULE Ordinary ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'={next}\nP == {property}\n====\n"
            ),
        )?;
        std::fs::write(
            file.with_extension("cfg"),
            "INIT Init\nNEXT Next\nPROPERTY P\n",
        )?;
        Ok(Self {
            _directory: directory,
            file,
        })
    }
    fn report(&self, workers: usize) -> Result<Value, Box<dyn std::error::Error>> {
        let request = CheckRequest {
            file: self.file.clone(),
            compat_tlc: false,
            fp: Some(0),
            seed: Some(0),
            workers: Some(workers),
            // Check every grown level, so small graphs exercise real prefixes.
            periodic: dive_api::PeriodicSchedule {
                min_states: 2,
                growth_factor: 1,
            },
            ..CheckRequest::default()
        };
        Ok(dive_api::check_file(&request)?.to_json_with_exit_convention(ExitConvention::Tlc))
    }
}
fn mapped(value: &Value, status: Option<i32>, profile: bool) -> CheckMapping {
    check_result(
        &ProcessOutcome {
            exit_code: status,
            signal: None,
            timed_out: false,
            stdout: value.to_string(),
            stderr: String::new(),
            elapsed: Duration::ZERO,
        },
        profile,
    )
}
fn fact(value: &Value, status: i32) -> Result<OracleResult, Box<dyn std::error::Error>> {
    match mapped(value, Some(status), false) {
        CheckMapping::Result(result) if result.verdict.is_oracle_fact() => Ok(*result),
        _ => Err("expected actual classified deterministic result".into()),
    }
}
fn rejected(value: &Value, status: Option<i32>, profile: bool) {
    match mapped(value, status, profile) {
        CheckMapping::Refused(_) => {}
        CheckMapping::Result(result) => assert_eq!(result.verdict, Verdict::Unclassified),
    }
}
fn model(file: &Path) -> Result<SpecModel, Box<dyn std::error::Error>> {
    Ok(SpecModel::load(
        file,
        &file.with_extension("cfg"),
        &TraceOptions {
            compat_tlc: false,
            run: Some((0, 0)),
            ..TraceOptions::default()
        },
    )?)
}
const CHAIN: &str = "IF x<2 THEN x+1 ELSE x";

#[test]
fn actual_default_prefix_retains_partial_counts_and_requires_independent_replay() -> TestResult {
    let spec = Spec::new("<>(x=3)", CHAIN)?;
    let model = model(&spec.file)?;
    for width in [1, 4, 8] {
        let value = spec.report(width)?;
        assert_eq!(value["schema_version"], 2);
        assert_eq!(value["options"]["profile"], "dive");
        assert_eq!(value["options"]["lncheck"], "deterministic");
        assert_eq!(value["execution"]["capture_completed"], false);
        assert_eq!(value["execution"]["stage"], "prefix");
        assert_eq!(value["execution"]["level"], 1);
        assert_eq!(value["periodic"]["prefix_checks"], 1);
        let result = fact(&value, 13)?;
        assert_eq!(result.verdict, Verdict::LivenessViolation);
        assert_eq!(
            result.error_class.as_deref(),
            Some("TLC_TEMPORAL_PROPERTY_VIOLATED")
        );
        assert_eq!(result.exit_code, Some(13));
        assert_eq!(result.violated_property.as_deref(), Some("P"));
        assert_eq!(
            result.counts,
            Counts {
                distinct_states: Some(2),
                states_generated: Some(2),
                states_left_on_queue: Some(1),
                depth: Some(2),
            }
        );
        assert!(validate_counterexample(&model, &result, &result).validated);
        // A coherent display and producer marker never replace independent replay.
        let mut forged = value.clone();
        let mut document = TraceDoc::from_json(&forged["counterexample"]["trace"])?;
        document.states[0].insert("x".into(), json!(99));
        forged["counterexample"]["trace"] = document.to_json();
        forged["counterexample"]["states"][0]["values"]["x"] = json!(99);
        let forged = fact(&forged, 13)?;
        assert!(validate_counterexample(&model, &forged, &result).refuted);
        let mut claims = value.clone();
        claims["properties"] = json!(["Other"]);
        claims["property"] = json!("Other");
        claims["counterexample"]["trace"]["properties"] = json!(["Other"]);
        assert!(!validate_counterexample(&model, &fact(&claims, 13)?, &result).validated);
    }
    Ok(())
}

#[test]
fn actual_default_exhaustion_is_required_for_holds_and_final_words() -> TestResult {
    for (property, next, status, generated, distinct, depth) in [
        ("[]<>(x<=2)", CHAIN, 0, 4, 3, 3),
        ("<>(x=1)", "x", 13, 2, 1, 1),
    ] {
        let spec = Spec::new(property, next)?;
        let value = spec.report(1)?;
        assert_eq!(value["execution"]["capture_completed"], true);
        assert_eq!(value["execution"]["exhausted"], true);
        assert_eq!(value["periodic"]["exhausted"], true);
        assert_eq!(value["periodic"]["final_check"], true);
        let result = fact(&value, status)?;
        assert_eq!(
            result.counts,
            Counts {
                distinct_states: Some(distinct),
                states_generated: Some(generated),
                states_left_on_queue: Some(0),
                depth: Some(depth),
            }
        );
        if status == 0 {
            assert_eq!(result.verdict, Verdict::Success);
            assert!(result.trace.is_none());
        } else {
            assert_eq!(result.verdict, Verdict::LivenessViolation);
            assert!(validate_counterexample(&model(&spec.file)?, &result, &result).validated);
        }
        for (pointer, replacement) in [
            ("/execution/capture_completed", json!(false)),
            ("/execution/exhausted", json!(false)),
            ("/periodic/exhausted", json!(false)),
            ("/periodic/final_check", json!(false)),
            ("/statistics/left_on_queue", json!(1)),
        ] {
            let mut changed = value.clone();
            *changed.pointer_mut(pointer).ok_or("existing field")? = replacement;
            rejected(&changed, Some(status), false);
        }
    }
    Ok(())
}

#[test]
fn actual_default_typed_planning_error_retains_null_counts_and_exact_status() -> TestResult {
    let spec = Spec::new("<>1", CHAIN)?;
    let value = spec.report(1)?;
    assert_eq!(value["verdict"], "evaluation-error");
    assert_eq!(value["diagnostics"][0]["code"], "DIVE-L0023");
    assert_eq!(value["execution"]["stage"], "planning");
    assert_eq!(value["execution"]["capture_completed"], false);
    assert!(value["statistics"].is_null() && value["counterexample"].is_null());
    let result = fact(&value, 255)?;
    assert_eq!(result.verdict, Verdict::EvaluationError);
    assert_eq!(result.error_class.as_deref(), Some("TLC_EXPECTED_VALUE"));
    assert_eq!(result.counts, Counts::default());
    assert_eq!(result.exit_code, Some(255));
    assert!(result.trace.is_none());
    // Pre-search failure needs no invented check history or capture statistics.
    let mut no_history = value.clone();
    no_history["periodic"] = Value::Null;
    assert_eq!(fact(&no_history, 255)?.counts, Counts::default());
    for (pointer, replacement) in [
        ("/tlc/code", json!(2193)),
        ("/tlc/name", json!("TLC_SUCCESS")),
        ("/tlc/exit_status", json!(0)),
        ("/property", json!("P")),
    ] {
        let mut changed = value.clone();
        *changed
            .pointer_mut(pointer)
            .ok_or("existing failure field")? = replacement;
        rejected(&changed, Some(255), false);
    }
    let mut wrong = value;
    wrong["exit_code"] = json!(0);
    rejected(&wrong, Some(0), false);
    Ok(())
}

#[test]
fn actual_default_prefix_and_final_refusals_never_publish_a_word() -> TestResult {
    for (next, complete, checks) in [(CHAIN, false, 1), ("x", true, 0)] {
        let spec = Spec::new("<>[]x", next)?;
        let value = spec.report(1)?;
        assert_eq!(value["verdict"], "unsupported");
        assert_eq!(value["execution"]["stage"], "atoms");
        assert_eq!(value["execution"]["capture_completed"], complete);
        assert_eq!(value["periodic"]["final_check"], complete);
        assert_eq!(value["periodic"]["prefix_checks"], checks);
        assert_eq!(value["statistics"]["generated"], 2);
        assert_eq!(
            value["statistics"]["distinct"],
            if complete { 1 } else { 2 }
        );
        assert!(value["counterexample"].is_null() && value["property"].is_null());
        assert!(matches!(
            mapped(&value, Some(3), false),
            CheckMapping::Refused(_)
        ));
    }
    Ok(())
}

#[test]
fn malformed_prefix_metadata_profiles_statuses_and_claims_are_not_facts() -> TestResult {
    let spec = Spec::new("<>(x=3)", CHAIN)?;
    let value = spec.report(1)?;
    for (pointer, replacement) in [
        ("/execution/capture_completed", json!(true)),
        ("/execution/exhausted", json!(true)),
        ("/execution/stage", json!("complete")),
        ("/execution/level", json!(2)),
        ("/statistics/generated", json!(-1)),
        ("/statistics/depth", Value::Null),
        ("/statistics/left_on_queue", json!(0)),
        ("/periodic/min_states", Value::Null),
        ("/periodic/growth_factor", json!(0.5)),
        ("/periodic/prefix_checks", json!(0)),
        ("/periodic/last_level", Value::Null),
        ("/periodic/last_size", json!(99)),
        ("/periodic/final_check", json!(true)),
        ("/periodic/exhausted", json!(true)),
        ("/options/profile", json!("tlc")),
        ("/options/profile", json!("dive+overrides")),
        ("/options/lncheck", json!("final")),
        ("/options/lasso", json!("tlc")),
        ("/options/exit_codes", json!("native")),
        ("/tlc/code", json!(2193)),
        ("/tlc/name", json!("TLC_SUCCESS")),
        ("/tlc/verified", json!(false)),
        ("/tlc/exit_status", json!(0)),
        ("/exit_code", json!(0)),
        ("/counterexample/back_to", json!(0)),
        ("/counterexample/trace", Value::Null),
        ("/counterexample/states/0/values/x", json!(99)),
        ("/properties", json!(["P", "P"])),
        ("/property", json!("Other")),
    ] {
        let mut changed = value.clone();
        *changed
            .pointer_mut(pointer)
            .ok_or("existing prefix field")? = replacement;
        rejected(&changed, Some(13), false);
    }
    for status in [None, Some(0), Some(12), Some(255)] {
        rejected(&value, status, false);
    }
    rejected(&value, Some(13), true);
    let mut holds = value;
    holds["verdict"] = json!("success");
    holds["tlc"] = json!({"code":2193,"name":"TLC_SUCCESS","verified":true,"exit_status":0});
    holds["exit_code"] = json!(0);
    holds["counterexample"] = Value::Null;
    holds["property"] = Value::Null;
    rejected(&holds, Some(0), false);
    Ok(())
}
