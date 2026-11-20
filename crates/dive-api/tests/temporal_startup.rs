//! Startup zero solutions use actual bound temporal roles, before exploration.
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;
use std::path::{Path, PathBuf};

use dive_api::{CheckRequest, ExitConvention};
use dive_runtime::CheckOptions;
use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;
const CORPUS: &str = "temporal-demand-primary-probes";

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}
fn request(name: &str, tlc: bool, mode: refused::Mode, workers: usize) -> CheckRequest {
    CheckRequest {
        file: root().join(format!(
            "conformance/corpus/{CORPUS}/specifications/{name}/{name}.tla"
        )),
        compat_tlc: tlc,
        temporal_refusal: mode.refusal(),
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
/// `--lncheck final --lasso short`, which only the retired reference engine ran (#791, #910),
/// and otherwise `ordinary`.
fn assert_core_refusal(request: &CheckRequest, ordinary: &str) -> TestResult {
    let report = dive_api::check_file(request)?;
    let actual = report.to_json();
    let feature = if request.temporal_refusal.is_some() {
        refused::FINAL_SHORT
    } else {
        ordinary
    };
    refused::assert_refused(&actual, report.exit_code(), feature);
    assert_eq!(actual["schema_version"], 4);
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
            for mode in [refused::Mode::Ordinary, refused::Mode::FinalShort] {
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
            for mode in [refused::Mode::Ordinary, refused::Mode::FinalShort] {
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
        for mode in [refused::Mode::Ordinary, refused::Mode::FinalShort] {
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
fn startup_tautology_after_a_binding_print_is_a_named_refusal() {
    // The retired reference final route (#791) typed this start-up tautology
    // and kept its one binding Print; Core refuses the temporal validation
    // failure report by name, before any search and with no output.
    let text = "---- MODULE StartupPrint ----\nEXTENDS Naturals,TLC\nVARIABLE x\nBindingMarker == PrintT(\"BIND_SEEN\")\nASSUME PrintT(\"ASSUME_SEEN\")\nInit == x=0\nNext == x'=1-x\nBad == IF x=0 THEN 1 \\div 0=0 ELSE TRUE\nP == ([]<>Bad)=>TRUE\n====\n";
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let report = core_final::run(
            text,
            "INIT Init\nNEXT Next\nPROPERTY P\nCHECK_DEADLOCK FALSE\n",
            &options,
        );
        let actual = report.to_json();
        refused::assert_refused(
            &actual,
            report.exit_code(),
            refused::TEMPORAL_VALIDATION_FAILURE_REPORT,
        );
        assert_eq!(actual["output"], serde_json::json!([]), "{actual}");
        assert!(actual["statistics"].is_null(), "{actual}");
    }
}

#[test]
fn an_expired_run_budget_is_a_typed_stop_not_a_zero_solution_verdict() -> TestResult {
    for tlc in [false, true] {
        for mode in [refused::Mode::Ordinary, refused::Mode::FinalShort] {
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
