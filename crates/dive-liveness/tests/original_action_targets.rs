//! Actual constructor aliases keep TLC's source/selected-target observations.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite regression assertions"
)]
use dive_check::CheckOptions;
use dive_liveness::analyze::{self, Analysis, AnalysisError, Limits};
use std::sync::Arc;
const SOURCE: &str = include_str!("fixtures/constructor-alias.tla");
fn bound(next: &str, property: &str, options: &CheckOptions) -> dive_model::Model {
    let analysis = dive_sema::analyze_source(
        "ConstructorAlias.tla",
        SOURCE,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse(&format!("INIT Init\nNEXT {next}\nPROPERTY {property}\n"))
        .into_config()
        .expect("config");
    dive_model::bind(
        ir,
        &config,
        &dive_model::BindOptions {
            eval: options.eval,
            no_deadlock: options.no_deadlock,
            host: None,
        },
    )
    .expect("bind")
}
#[test]
fn actual_selected_record_errors_but_first_function_state_and_eliminated_controls_hold() {
    let oracle: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/constructor-alias/observations.json"))
            .expect("compact primary observations");
    let cases = oracle["expected_cases"].as_array().expect("five controls");
    assert_eq!(cases.len(), 5);
    for store in [
        dive_store::StoreMode::Fingerprint,
        dive_store::StoreMode::Exact,
    ] {
        let options = CheckOptions {
            store,
            ..CheckOptions::tlc(0, 0)
        };
        for case in cases {
            let next = case["next"].as_str().expect("Next name");
            let property = case["property"].as_str().expect("property name");
            let generated = case["generated"].as_u64().expect("generated count");
            let error = case["physical_exit"] == 255;
            assert_eq!(case["distinct"], 1);
            assert_eq!(case["depth"], 1);
            let model = bound(next, property, &options);
            match analyze::analyze_final_with_action_errors(&model, &options, Limits::default()) {
                Ok(Analysis::Holds(capture)) => {
                    assert!(!error);
                    assert_eq!(
                        (
                            capture.stats.generated,
                            capture.stats.distinct,
                            capture.stats.depth
                        ),
                        (generated, 1, 1)
                    );
                }
                Err(failure) => {
                    assert!(error, "{next}/{property}: {failure}");
                    let action = failure
                        .action_evaluation()
                        .expect("actual typed action cause");
                    assert_eq!(action.cause().ec(), dive_eval::Ec::General);
                    assert_eq!(action.cause().class(), dive_eval::ErrorClass::Eval);
                    assert!(!action.cause().is_eval_exception());
                    assert!(action.core_cause().is_some());
                    assert!(action.direct_operand());
                    assert_eq!((action.property(), action.properties()), ("ActionCheck", 1));
                    assert!(action.source().location.contains("ConstructorAlias"));
                    assert_eq!(failure.failure().stage, analyze::Stage::Atoms);
                    let capture = failure
                        .failure()
                        .exploration
                        .as_ref()
                        .expect("completed capture");
                    assert_eq!(
                        (
                            capture.stats.generated,
                            capture.stats.distinct,
                            capture.stats.depth
                        ),
                        (generated, 1, 1)
                    );
                }
                other => panic!("unexpected result: {other:?}"),
            }
        }
    }
}
#[test]
fn legacy_error_shapes_and_default_value_semantics_remain_available() {
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        let model = bound("Next", "ActionCheck", &options);
        let result = analyze::analyze_final(&model, &options, Limits::default());
        if options.profile == dive_eval::Profile::Tlc {
            assert!(
                matches!(result, Err(AnalysisError::Atom { property, source, detail }) if property=="ActionCheck" && source.location.contains("ConstructorAlias") && detail.contains("GENERAL"))
            );
        } else {
            assert!(matches!(result, Ok(Analysis::Holds(_))));
        }
    }
}
#[test]
fn unchanged_cell_guard_refuses_before_action_evaluation() {
    let options = CheckOptions::tlc(0, 0);
    for property in ["ActionCheck", "EliminatedAction", "StateControl"] {
        let model = bound("Next", property, &options);
        let failure = analyze::analyze_final_with_action_errors(
            &model,
            &options,
            Limits {
                atom_cells: 0,
                ..Limits::default()
            },
        )
        .expect_err("original cell guard");
        assert!(matches!(
            failure.failure().error,
            AnalysisError::Limit("atom cells")
        ));
        assert!(failure.action_evaluation().is_none());
    }
}

#[test]
fn no_graph_ceiling_preserves_the_original_selected_action_error() {
    use dive_liveness::analyze::GraphLabelPolicy;
    use dive_liveness::product::AcceptancePolicy;
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        let model = bound("Next", "ActionCheck", &options);
        let result = analyze::analyze_final_with_policies(
            &model,
            &options,
            Limits {
                atom_cells: 0,
                ..Limits::default()
            },
            AcceptancePolicy::compact_default(),
            GraphLabelPolicy::NoCellLimit,
        );
        if options.profile == dive_eval::Profile::Tlc {
            let failed = result.expect_err("real action evaluation failure precedes attribution");
            assert_eq!(failed.failure().stage, analyze::Stage::Atoms);
            let action = failed.action_evaluation().expect("original typed cause");
            assert_eq!(action.cause().class(), dive_eval::ErrorClass::Eval);
            assert_eq!(action.cause().ec(), dive_eval::Ec::General);
            assert_eq!(action.property(), "ActionCheck");
        } else {
            assert!(matches!(result, Ok(Analysis::Holds(_))));
        }
    }
}
