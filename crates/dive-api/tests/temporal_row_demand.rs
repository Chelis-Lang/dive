//! Ordinary row errors require demand, actual prefix facts and fresh validation.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use dive_api::{CheckRequest, ExitConvention};
use dive_check::CheckOptions;
use dive_liveness::analyze::{Limits, StateEvaluationSite};
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

fn source_input(
    body: &str,
    extra_config: &str,
) -> Result<(Arc<dive_ir::Ir>, dive_config::ModelConfig), Box<dyn std::error::Error>> {
    let source = format!(
        "---- MODULE RowControl ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLE x\n{body}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("RowControl.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let config = dive_config::parse(&format!("INIT Init\nNEXT Next\nPROPERTY P\n{extra_config}"))
        .into_config()
        .ok_or("configuration")?;
    Ok((Arc::new(dive_ir::lower(&analysis)?), config))
}

fn final_no_cells(
    ir: &Arc<dive_ir::Ir>,
    config: &dive_config::ModelConfig,
    options: &CheckOptions,
) -> Result<dive_api::temporal::Report, dive_api::temporal::Failure> {
    dive_api::temporal::check_final_with_policies(
        ir,
        config,
        options,
        Limits::default(),
        dive_liveness::product::AcceptancePolicy::Legacy,
        dive_liveness::analyze::GraphLabelPolicy::NoCellLimit,
    )
}

#[test]
fn the_five_pinned_row_errors_match_original_cause_prefix_and_finite_word() -> TestResult {
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
fn a_source_bound_api_failure_retains_the_original_typed_cause_and_real_row() -> TestResult {
    let name = "CurrentStateBinPriority";
    let directory = root().join(format!(
        "conformance/corpus/temporal-demand-primary-probes/specifications/{name}"
    ));
    let source = std::fs::read_to_string(directory.join(format!("{name}.tla")))?;
    let analysis = dive_sema::analyze_source(
        &format!("{name}.tla"),
        &source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis)?);
    let config = dive_config::parse(&std::fs::read_to_string(
        directory.join(format!("{name}.cfg")),
    )?)
    .into_config()
    .ok_or("configuration")?;
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        let failure = final_no_cells(&ir, &config, &options)
            .err()
            .ok_or("expected original row error")?;
        let state = failure.state_evaluation().ok_or("typed row cause")?;
        assert_eq!(state.site(), StateEvaluationSite::CaptureRow);
        assert_eq!(
            state.core_cause().map(|cause| cause.ec().code()),
            Some(2179)
        );
        assert!(state.cause().origin().is_some());
        let row = state.capture_row().ok_or("opaque row evidence")?;
        assert_eq!(row.source(), 0);
        assert_eq!(
            row.result().stats.map(|stats| (
                stats.distinct,
                stats.generated,
                stats.left_on_queue,
                stats.depth
            )),
            Some((2, 2, 1, 2))
        );
        assert!(matches!(
            &row.result().verdict,
            dive_check::Verdict::Unsupported { .. }
        ));
        assert_eq!(failure.capture_completed(), Some(false));
        assert!(failure.action_evaluation().is_none());
    }
    Ok(())
}

#[test]
fn initial_invariant_and_next_errors_keep_priority_before_any_row_decision() -> TestResult {
    for (next, invariant) in [("x'=1", true), ("x'=1 \\div 0", false)] {
        let (ir, config) = source_input(
            &format!("Init == x=0\nNext == {next}\nInv == x=1\nP == []<>(1 \\div (x-x)=0)"),
            if invariant { "INVARIANT Inv\n" } else { "" },
        )?;
        for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
            let report = final_no_cells(&ir, &config, &options)?;
            let dive_api::temporal::Outcome::Safety(result) = report.outcome() else {
                return Err("earlier safety stop must not become a temporal decision".into());
            };
            if invariant {
                assert_eq!(
                    result.verdict,
                    dive_check::Verdict::InvariantViolated {
                        property: "Inv".into(),
                        initial: true
                    }
                );
            } else {
                assert_eq!(result.verdict.kind(), "evaluation-error");
                assert_eq!(result.verdict.tlc().map(|class| class.code), Some(2179));
            }
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

fn assert_stateful_filter_refusal(result: &dive_check::CheckResult) {
    assert_eq!(
        result.verdict,
        dive_check::Verdict::Unsupported {
            feature: "graph capture with stateful checking: side effects inside a set filter TLC evaluates lazily".into(),
        }
    );
    assert!(result.stats.is_none(), "no search counters fabricated");
    assert!(
        result.counterexample.is_none(),
        "no capture word fabricated"
    );
    assert_eq!(result.workers.used, 0);
    assert_eq!(result.output, Vec::<String>::new());
}

#[test]
fn uncertified_future_provider_uses_the_legacy_route_without_extra_callbacks() -> TestResult {
    let (ir, config) = source_input(
        "Init == x={p \\in ({0} \\X {0}): PrintT(p[1]=0)}\nNext == UNCHANGED x\nP == []<>(<<0,0>> \\in x)",
        "",
    )?;
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        // A separate direct Core proves the provider genuinely prints. It and
        // its retained value are dropped before either graph-capture route.
        assert_real_filter_callback(&ir, &options)?;
        let model = dive_model::bind(
            ir.clone(),
            &config,
            &dive_model::BindOptions {
                eval: options.eval,
                symmetry_liveness: options.symmetry_liveness,
                no_deadlock: options.no_deadlock,
                host: None,
            },
        )?;
        let reference = dive_liveness::analyze::analyze_final_with_policies(
            &model,
            &options,
            Limits::default(),
            dive_liveness::product::AcceptancePolicy::Legacy,
            dive_liveness::analyze::GraphLabelPolicy::Cells(Limits::default().atom_cells),
        )
        .err()
        .ok_or("legacy capture must retain its presearch effect refusal")?;
        let dive_liveness::analyze::AnalysisError::Exploration(reference_result) =
            &reference.failure().error
        else {
            return Err("legacy graph capture refusal".into());
        };
        assert_stateful_filter_refusal(reference_result);
        assert!(reference.failure().exploration.is_none());
        assert!(reference.state_evaluation().is_none());
        assert!(reference.action_evaluation().is_none());
        // The public API has always transported capture refusals as an Ok
        // report containing Outcome::Safety, while the analyzer returns Err.
        let actual = final_no_cells(&ir, &config, &options)?;
        let dive_api::temporal::Outcome::Safety(actual_result) = actual.outcome() else {
            return Err(format!(
                "unknown producer must retain its presearch effect refusal: {:?}",
                actual.outcome()
            )
            .into());
        };
        assert_stateful_filter_refusal(actual_result);
        assert_eq!(actual_result.verdict, reference_result.verdict);
        assert_eq!(actual_result.output, reference_result.output);
        assert_eq!(actual_result.stats, reference_result.stats);
    }
    Ok(())
}

#[test]
fn an_owned_budget_stop_never_becomes_a_semantic_row_error() -> TestResult {
    let (ir, config) = source_input(
        "Init == x=0\nNext == x'=1-x\nP == []<>(1 \\div (x-x)=0)",
        "",
    )?;
    for mut options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        options.budget = dive_check::budget::Budget::new(Some(std::time::Duration::ZERO), None);
        let failure = final_no_cells(&ir, &config, &options)
            .err()
            .ok_or("owned stop required")?;
        assert!(matches!(
            failure.error(),
            dive_api::temporal::Error::Analysis(dive_liveness::analyze::AnalysisError::Stopped(_))
        ));
        assert!(failure.state_evaluation().is_none());
        assert!(failure.action_evaluation().is_none());
    }
    Ok(())
}

#[test]
fn all_finite_label_policies_keep_legacy_admission_before_label_errors() -> TestResult {
    use dive_liveness::analyze::{AnalysisError, GraphLabelPolicy};
    let (ir, config) = source_input(
        "Init == x=0\nNext == x'=1-x\nP == []<>(1 \\div (x-x)=0)",
        "",
    )?;
    for options in [CheckOptions::dive(0), CheckOptions::tlc(0, 0)] {
        for cells in [0, usize::MAX] {
            let failure = dive_api::temporal::check_final_with_policies(
                &ir,
                &config,
                &options,
                Limits::default(),
                dive_liveness::product::AcceptancePolicy::Legacy,
                GraphLabelPolicy::Cells(cells),
            )
            .err()
            .ok_or("legacy postcapture failure")?;
            assert_eq!(failure.capture_completed(), Some(true));
            let capture = failure.exploration().ok_or("whole legacy capture")?;
            assert_eq!(
                (
                    capture.stats.distinct,
                    capture.stats.generated,
                    capture.stats.left_on_queue
                ),
                (2, 3, 0)
            );
            if cells == 0 {
                assert!(matches!(
                    failure.error(),
                    dive_api::temporal::Error::Analysis(AnalysisError::Limit(_))
                ));
                assert!(
                    failure.state_evaluation().is_none(),
                    "bound stops before label evaluation"
                );
            } else {
                let state = failure.state_evaluation().ok_or("legacy typed cause")?;
                assert_eq!(state.site(), StateEvaluationSite::FinalLabels);
                assert!(state.capture_row().is_none());
            }
        }
        let row = final_no_cells(&ir, &config, &options)
            .err()
            .ok_or("real source-row error")?;
        assert_eq!(row.capture_completed(), Some(false));
        assert!(
            row.state_evaluation()
                .and_then(|state| state.capture_row())
                .is_some()
        );
    }
    Ok(())
}
