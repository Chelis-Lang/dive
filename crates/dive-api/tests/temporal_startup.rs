//! Startup zero solutions use actual bound temporal roles, before exploration.
#[path = "support/refused.rs"]
mod refused;
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
/// Asserts Core's named refusal for `request`: bounded final checking for
/// `FinalShort`, which only the retired reference engine ran (#791, #910),
/// and otherwise `ordinary`.
fn assert_core_refusal(request: &CheckRequest, ordinary: &str) -> TestResult {
    let report = dive_api::check_file_engine(request, dive_api::Engine::Core)?;
    let actual = report.to_json();
    let feature = if request.temporal == TemporalMode::FinalShort {
        refused::FINAL_SHORT
    } else {
        ordinary
    };
    refused::assert_refused(&actual, report.exit_code(), feature);
    assert_eq!(actual["schema_version"], 3);
    assert!(actual["statistics"].is_null(), "{actual}");
    assert!(report.temporal_result().is_none());
    Ok(())
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
        // TLC reports its start-up tautology (2253, exit 77) before ASSUMEs,
        // Init and invariants. Core runs the same start-up check first (#918)
        // but does not build that report: it refuses by name.
        let expected = oracle(name)?;
        assert_eq!(expected["result"]["exit_code"], 77);
        for tlc in [false, true] {
            for mode in [TemporalMode::Ordinary, TemporalMode::FinalShort] {
                for workers in [1, 4] {
                    assert_core_refusal(
                        &request(name, tlc, mode, workers),
                        refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
                    )?;
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
        // TLC succeeds and prints INIT_SEEN; Core refuses the Print with
        // temporal properties, never a zero-solution verdict.
        let expected = oracle(name)?;
        assert_eq!(expected["result"]["verdict"], "success");
        for tlc in [false, true] {
            for mode in [TemporalMode::Ordinary, TemporalMode::FinalShort] {
                assert_core_refusal(
                    &request(name, tlc, mode, 1),
                    "Print or PrintT with temporal properties (core)",
                )?;
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
            let report =
                dive_api::check_file_engine(&request(name, tlc, mode, 1), dive_api::Engine::Core)?;
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
            let report = dive_api::check_file_engine(&req, dive_api::Engine::Core)?;
            let actual = report.to_json_with_exit_convention(ExitConvention::Tlc);
            assert_eq!(actual["verdict"], "stopped");
            assert_eq!(actual["stopped"]["resource"], "time");
            assert!(actual["tlc"].is_null());
            assert!(actual["counterexample"].is_null());
        }
    }
    Ok(())
}
