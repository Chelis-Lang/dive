//! Startup zero solutions use actual bound temporal roles, before exploration.
use std::path::{Path, PathBuf};
use std::sync::Arc;

use dive_api::{CheckRequest, ExitConvention, TemporalMode};
use dive_check::CheckOptions;
use dive_liveness::analyze::{AnalysisError, Limits};
use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const CORPUS: &str = "temporal-demand-primary-probes";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn request(name: &str, tlc: bool, mode: TemporalMode, workers: usize) -> CheckRequest {
    CheckRequest {
        file: root().join(format!(
            "conformance/corpus/{CORPUS}/specifications/{name}/{name}.tla"
        )),
        compat_tlc: tlc,
        temporal: mode,
        workers: Some(workers),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    }
}
fn oracle(name: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&std::fs::read(root().join(format!(
        "conformance/golden/tlc-2026.10.06.014338/{CORPUS}/specifications/{name}/{name}.cfg.json"
    )))?)?)
}
fn zero_document(actual: &Value, expected: &Value) {
    assert_eq!(actual["schema_version"], 3);
    assert_eq!(actual["verdict"], expected["verdict"]);
    assert_eq!(actual["tlc"]["code"], expected["verdict_code"]);
    assert_eq!(actual["tlc"]["name"], expected["error_class"]);
    assert_eq!(actual["tlc"]["exit_status"], expected["exit_code"]);
    assert_eq!(actual["exit_code"], 77);
    assert_eq!(actual["tlc"]["verified"], true);
    assert!(actual["property"].is_null());
    assert!(actual["counterexample"].is_null());
    assert!(actual["error_site"].is_null());
    assert!(actual["statistics"].is_null(), "{actual}");
    assert_eq!(actual["options"]["workers"]["used"], 0);
    assert_eq!(actual["execution"]["capture_completed"], false);
    assert_eq!(actual["output"], serde_json::json!([]));
}

#[test]
fn pinned_zero_solution_startup_precedes_assumptions_init_and_invariants() -> TestResult {
    for name in [
        "UnusedDemandTautology",
        "TautologyBeforeFalseAssume",
        "TautologyBeforePrintAssume",
        "TautologyBeforeInitError",
        "TautologyBeforeInvariant",
        "StartupRoleSplitInitial",
        "StartupRoleSplitBeforeFalseAssume",
    ] {
        let expected = oracle(name)?;
        for tlc in [false, true] {
            for mode in [TemporalMode::Ordinary, TemporalMode::FinalShort] {
                for workers in [1, 4] {
                    let report = dive_api::check_file(&request(name, tlc, mode, workers))?;
                    assert!(report.result().is_none());
                    assert!(report.temporal_result().is_none());
                    zero_document(
                        &report.to_json_with_exit_convention(ExitConvention::Tlc),
                        &expected["result"],
                    );
                }
            }
        }
    }
    Ok(())
}

#[test]
fn semantically_true_temporal_forms_keep_their_actual_nonzero_startup_route() -> TestResult {
    for name in [
        "EventuallyTrueNotSyntacticZero",
        "EventuallyAlwaysTrueNotSyntacticZero",
    ] {
        let expected = oracle(name)?;
        for tlc in [false, true] {
            for mode in [TemporalMode::Ordinary, TemporalMode::FinalShort] {
                let report = dive_api::check_file(&request(name, tlc, mode, 1))?;
                let actual = report.to_json_with_exit_convention(ExitConvention::Tlc);
                assert_eq!(actual["verdict"], expected["result"]["verdict"]);
                assert_eq!(actual["tlc"]["code"], expected["result"]["verdict_code"]);
                assert_eq!(actual["exit_code"], 0);
                for (field, original) in [
                    ("distinct", "distinct_states"),
                    ("generated", "states_generated"),
                    ("depth", "depth"),
                ] {
                    assert_eq!(
                        actual["statistics"][field],
                        expected["result"]["counts"][original]
                    );
                }
                assert!(
                    actual["output"]
                        .as_array()
                        .ok_or("output")?
                        .iter()
                        .any(|line| line.as_str().is_some_and(|s| s.contains("INIT_SEEN")))
                );
            }
        }
    }
    Ok(())
}

#[test]
fn earlier_binding_errors_keep_the_original_code_and_never_become_tautology() -> TestResult {
    let name = "BindingBeforeTautology";
    let expected = oracle(name)?;
    for tlc in [false, true] {
        for mode in [TemporalMode::Ordinary, TemporalMode::FinalShort] {
            let report = dive_api::check_file(&request(name, tlc, mode, 1))?;
            let actual = report.to_json_with_exit_convention(ExitConvention::Tlc);
            assert_eq!(actual["tlc"]["code"], expected["result"]["verdict_code"]);
            assert_eq!(actual["tlc"]["name"], expected["result"]["error_class"]);
            assert_eq!(actual["exit_code"], expected["result"]["exit_code"]);
            assert!(actual["statistics"]["distinct"].is_null());
            assert!(actual["counterexample"].is_null());
        }
    }
    Ok(())
}

#[test]
fn typed_startup_failure_retains_one_binding_print_and_has_no_row_or_eval_error() -> TestResult {
    let text = "---- MODULE StartupPrint ----\nEXTENDS Naturals,TLC\nVARIABLE x\nBindingMarker == PrintT(\"BIND_SEEN\")\nASSUME PrintT(\"ASSUME_SEEN\")\nInit == x=0\nNext == x'=1-x\nBad == IF x=0 THEN 1 \\div 0=0 ELSE TRUE\nP == ([]<>Bad)=>TRUE\n====\n";
    let analysis =
        dive_sema::analyze_source("StartupPrint.tla", text, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nPROPERTY P\nCHECK_DEADLOCK FALSE\n")
        .into_config()
        .ok_or("config")?;
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let failure =
            dive_api::temporal::check_final_with_evidence(&ir, &cfg, &options, Limits::default())
                .err()
                .ok_or("startup failure")?;
        assert!(matches!(
            failure.error(),
            dive_api::temporal::Error::Analysis(AnalysisError::StartupTautology { .. })
        ));
        assert_eq!(failure.capture_completed(), Some(false));
        assert!(failure.exploration().is_none());
        assert!(failure.state_evaluation().is_none());
        assert!(failure.action_evaluation().is_none());
        assert_eq!(failure.binding_output().len(), 1);
        assert!(
            matches!(&failure.binding_output()[0], dive_model::Message::Print(line) if line.contains("BIND_SEEN"))
        );
    }
    Ok(())
}

#[test]
fn an_expired_run_budget_is_a_typed_stop_not_a_zero_solution_verdict() -> TestResult {
    for tlc in [false, true] {
        for mode in [TemporalMode::Ordinary, TemporalMode::FinalShort] {
            let mut req = request("UnusedDemandTautology", tlc, mode, 1);
            req.time_limit = dive_api::BudgetSetting::Explicit(std::time::Duration::ZERO);
            let report = dive_api::check_file(&req)?;
            let actual = report.to_json_with_exit_convention(ExitConvention::Tlc);
            assert_eq!(actual["verdict"], "stopped");
            assert_eq!(actual["stopped"]["resource"], "time");
            assert!(actual["tlc"].is_null());
            assert!(actual["counterexample"].is_null());
        }
    }
    Ok(())
}
