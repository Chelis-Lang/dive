//! Ordinary row errors require demand, actual prefix facts and fresh validation.
#[path = "support/core_final.rs"]
mod core_final;
#[path = "support/refused.rs"]
mod refused;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dive_api::{CheckRequest, ExitConvention};
use dive_runtime::CheckOptions;
use serde_json::Value;

type TestResult = Result<(), Box<dyn std::error::Error>>;

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn report(corpus: &str, name: &str, tlc: bool) -> Result<Value, Box<dyn std::error::Error>> {
    let result = dive_api::check_file(&CheckRequest {
        file: root().join(format!(
            "conformance/corpus/{corpus}/specifications/{name}/{name}.tla"
        )),
        compat_tlc: tlc,
        workers: Some(1),
        seed: Some(0),
        fp: Some(0),
        ..CheckRequest::default()
    })?;
    Ok(result.to_json_with_exit_convention(ExitConvention::Tlc))
}

fn golden(corpus: &str, name: &str) -> Result<Value, Box<dyn std::error::Error>> {
    Ok(serde_json::from_slice(&std::fs::read(root().join(format!(
        "conformance/golden/tlc-2026.10.06.014338/{corpus}/specifications/{name}/{name}.cfg.json"
    )))?)?)
}

fn row_source(body: &str) -> String {
    format!(
        "---- MODULE RowControl ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLE x\n{body}\n====\n"
    )
}

fn row_config(extra_config: &str) -> String {
    format!("INIT Init\nNEXT Next\nPROPERTY P\n{extra_config}")
}

/// A temporal source row whose first error is in `Next`.
const UNCERTIFIED_SOURCE_ROW: &str =
    "singleton temporal source-row primary constructor is not certified";
/// An effect inside a lazily evaluated constructor.
const LAZY_EFFECT: &str = "Print, PrintT, RandomElement, TLCSet or TLCGet inside a function constructor, set filter, set map or operator argument";

/// Asserts that `report` is Core's named refusal of `feature`.
fn assert_core_refused(report: &dive_api::CheckReport, feature: &str) {
    refused::assert_refused(&report.to_json(), report.exit_code(), feature);
}

#[test]
fn the_five_pinned_row_errors_match_original_cause_prefix_and_finite_word() -> TestResult {
    // Only the retired reference engine reported these liveness-atom errors
    // (#791, #910); Core refuses to build the report rather than claim the
    // property. Every case Core does report must match the pinned TLC row.
    let mut refusals = Vec::new();
    for (corpus, name) in [
        ("selectseq-concat-type-capability", "ConcatFirstTypeError"),
        ("selectseq-concat-type-capability", "ConcatSecondTypeError"),
        (
            "selectseq-concat-type-capability",
            "RightChildErrorBeforeKernel",
        ),
        ("liveness-atom-error-timing", "DelayedSiblingStateError"),
        ("temporal-demand-primary-probes", "CurrentStateBinPriority"),
    ] {
        let oracle = golden(corpus, name)?;
        let expected = &oracle["result"];
        for tlc in [false, true] {
            let actual = report(corpus, name, tlc)?;
            if actual["verdict"] == "unsupported" {
                let exit = u8::try_from(actual["exit_code"].as_u64().unwrap_or(255))?;
                refused::assert_refused(&actual, exit, refused::TEMPORAL_FAILURE_REPORT);
                refusals.push(format!("{name}/{tlc}"));
                continue;
            }
            assert_eq!(
                actual["verdict"], expected["verdict"],
                "{name}/{tlc}: {actual}"
            );
            assert_eq!(
                actual["tlc"]["code"], expected["verdict_code"],
                "{name}/{tlc}"
            );
            assert_eq!(
                actual["tlc"]["name"], expected["error_class"],
                "{name}/{tlc}"
            );
            assert_eq!(
                actual["tlc"]["exit_status"], expected["exit_code"],
                "{name}/{tlc}"
            );
            assert_eq!(actual["property"], expected["violated_property"]);
            for (field, oracle_field) in [
                ("distinct", "distinct_states"),
                ("generated", "states_generated"),
                ("left_on_queue", "states_left_on_queue"),
                ("depth", "depth"),
            ] {
                assert_eq!(
                    actual["statistics"][field], expected["counts"][oracle_field],
                    "{name}/{tlc}/{field}"
                );
            }
            assert_eq!(actual["schema_version"], 3);
            assert_eq!(actual["tlc"]["verified"], true);
            assert_eq!(actual["error_site"], "temporal-state-row");
            assert_eq!(actual["execution"]["capture_completed"], false);
            assert_eq!(actual["counterexample"]["validation"], "valid");
            assert_eq!(actual["counterexample"]["ending"], "finite");
            assert_eq!(
                actual["counterexample"]["length"],
                expected["trace"]["length"]
            );
            assert!(actual["counterexample"]["trace"]["properties"].is_null());
        }
    }
    // Today Core refuses all five in both profiles.
    assert_eq!(refusals.len(), 10, "{refusals:?}");
    Ok(())
}

#[test]
fn tautology_startup_and_tableau_init_are_not_relabelled_as_certified_rows() -> TestResult {
    for (corpus, name) in [
        ("temporal-demand-primary-probes", "UnusedDemandTautology"),
        ("liveness-atom-error-timing", "TableauInitAtomError"),
    ] {
        for tlc in [false, true] {
            let actual = report(corpus, name, tlc)?;
            if name == "UnusedDemandTautology" {
                // Only the retired reference engine reported TLC's startup
                // error here (2253, exit 77) (#791, #910); Core refuses.
                let exit = u8::try_from(actual["exit_code"].as_u64().unwrap_or(255))?;
                refused::assert_refused(&actual, exit, refused::TEMPORAL_VALIDATION_FAILURE_REPORT);
                assert!(actual["error_site"].is_null());
                continue;
            }
            assert_eq!(actual["verdict"], "unsupported", "{name}/{tlc}: {actual}");
            assert!(actual["tlc"].is_null());
            assert!(actual["error_site"].is_null());
            assert!(actual["counterexample"].is_null());
        }
    }
    let startup = golden("temporal-demand-primary-probes", "UnusedDemandTautology")?;
    assert_eq!(startup["result"]["verdict_code"], 2253);
    assert_eq!(startup["result"]["exit_code"], 77);
    assert!(startup["result"]["counts"]["distinct_states"].is_null());
    Ok(())
}

#[test]
fn existing_holding_and_lasso_controls_keep_independent_validation() -> TestResult {
    for (name, verdict) in [
        ("CapturedConcatHolds", "success"),
        ("CapturedConcatFails", "liveness-violation"),
        ("EmptySkipsCallbackError", "success"),
    ] {
        for tlc in [false, true] {
            let actual = report("selectseq-concat-type-capability", name, tlc)?;
            assert_eq!(actual["verdict"], verdict, "{name}/{tlc}: {actual}");
            if verdict == "liveness-violation" {
                assert_eq!(actual["counterexample"]["validation"], "valid");
                assert_eq!(actual["property"], "P");
            }
        }
    }
    Ok(())
}

#[test]
fn initial_invariant_and_next_errors_keep_priority_before_any_row_decision() -> TestResult {
    for (next, invariant) in [("x'=1", true), ("x'=1 \\div 0", false)] {
        let source = row_source(&format!(
            "Init == x=0\nNext == {next}\nInv == x=1\nP == []<>(1 \\div (x-x)=0)"
        ));
        let config = row_config(if invariant { "INVARIANT Inv\n" } else { "" });
        for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            if !invariant {
                // TLC reports the Next evaluation error (2179). Core refuses
                // this uncertified source row by name, never a verdict.
                let report = core_final::run(&source, &config, &options);
                assert_core_refused(&report, UNCERTIFIED_SOURCE_ROW);
                continue;
            }
            let run =
                core_final::check(&source, &config, &options).map_err(|e| e.json().to_string())?;
            let core_final::Outcome::Safety(result) = run.outcome() else {
                return Err("earlier safety stop must not become a temporal decision".into());
            };
            assert_eq!(
                result.verdict,
                dive_runtime::Verdict::InvariantViolated {
                    property: "Inv".into(),
                    initial: true
                }
            );
        }
    }
    Ok(())
}

fn assert_real_filter_callback(ir: &Arc<dive_ir::Ir>, options: &CheckOptions) -> TestResult {
    let evaluator = dive_eval::Evaluator::new(dive_eval::Model::new(ir.clone()), options.eval)?;
    let init = ir.root_def("Init").ok_or("Init")?;
    let dive_ir::Expr::Builtin {
        op: dive_ir::BuiltinOp::Eq,
        args,
    } = ir.expr(ir.def(init).body)
    else {
        return Err("actual Init equality fixture".into());
    };
    let rhs = *args.get(1).ok_or("Init filter RHS")?;
    let value = evaluator.eval_in(init, rhs, &dive_eval::St::constant())?;
    let dive_value::Value::Set(set) = &value else {
        return Err("actual set provider".into());
    };
    assert!(
        set.as_deferred().is_some(),
        "real retained evaluator filter"
    );
    assert_eq!(evaluator.take_output(), Vec::<String>::new());
    assert!(dive_value::tlc::member(
        &dive_value::Value::tuple(vec![0.into(), 0.into()]),
        &value,
        options.eval.ctx,
    )?);
    assert_eq!(evaluator.take_output(), vec!["TRUE".to_owned()]);
    Ok(())
}

#[test]
fn uncertified_future_provider_is_refused_without_extra_callbacks() -> TestResult {
    let source = row_source(
        "Init == x={p \\in ({0} \\X {0}): PrintT(p[1]=0)}\nNext == UNCHANGED x\nP == []<>(<<0,0>> \\in x)",
    );
    let analysis =
        dive_sema::analyze_source("RowControl.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        // A separate direct evaluator proves the provider genuinely prints.
        assert_real_filter_callback(&ir, &options)?;
        let report = core_final::run(&source, &row_config(""), &options);
        assert_core_refused(&report, LAZY_EFFECT);
        assert_eq!(report.to_json()["output"], serde_json::json!([]));
    }
    Ok(())
}

#[test]
fn an_owned_budget_stop_never_becomes_a_semantic_row_error() -> TestResult {
    let source = row_source("Init == x=0\nNext == x'=1-x\nP == []<>(1 \\div (x-x)=0)");
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let files = core_final::Files::new(&source, &row_config(""));
        let request = CheckRequest {
            time_limit: dive_api::BudgetSetting::Explicit(std::time::Duration::ZERO),
            ..files.request(&options)
        };
        let report = dive_api::check_file(&request)?;
        let json = report.to_json();
        assert_eq!(json["verdict"], "stopped", "{json}");
        assert!(json["error_site"].is_null(), "{json}");
        assert!(json["counterexample"].is_null(), "{json}");
        // Without a budget the label error is a temporal failure, whose
        // report Core does not build.
        let report = core_final::run(&source, &row_config(""), &options);
        assert_core_refused(&report, refused::TEMPORAL_FAILURE_REPORT);
    }
    Ok(())
}
