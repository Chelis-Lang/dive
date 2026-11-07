//! Pinned strong-fairness controls and independent original-formula replay.
#![allow(clippy::expect_used, clippy::panic, reason = "fixture assertions")]
use dive_liveness::analyze::{Analysis, Limits, analyze_final};
use dive_runtime::CheckOptions;
use dive_trace::{Ending, TraceDoc, Validation};
use std::sync::Arc;

#[test]
fn pinned_solution_controls_keep_all_properties_and_validated_witnesses() {
    let source = include_str!("fixtures/solutions/Solutions.tla");
    let analysis =
        dive_sema::analyze_source("Solutions.tla", source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    for options in [CheckOptions::tlc(0, 0), CheckOptions::dive(0)] {
        for property in ["Hold", "Fail", "Hold Fail FailOther", "Residual"] {
            let config = dive_config::parse(&format!(
                "CONSTANT N = 3\nSPECIFICATION Spec\nPROPERTY {property}\n"
            ))
            .into_config()
            .expect("cfg");
            let model = dive_model::bind(
                ir.clone(),
                &config,
                &dive_model::BindOptions {
                    eval: options.eval,
                    symmetry_liveness: options.symmetry_liveness,
                    no_deadlock: options.no_deadlock,
                    host: None,
                },
            )
            .expect("model");
            let result = analyze_final(&model, &options, Limits::default()).expect("analysis");
            let statistics = match &result {
                Analysis::Holds(captured) => &captured.stats,
                Analysis::Candidate(candidate) => &candidate.exploration.stats,
            };
            assert_eq!(
                (statistics.generated, statistics.distinct, statistics.depth),
                (25, 8, 4),
                "{property}"
            );
            if property == "Hold" {
                assert!(matches!(result, Analysis::Holds(_)));
                continue;
            }
            let Analysis::Candidate(candidate) = result else {
                panic!("expected violated {property}");
            };
            if property == "Hold Fail FailOther" {
                assert_eq!(candidate.properties, ["Fail", "FailOther"]);
            }
            let values = candidate
                .states
                .iter()
                .map(|state| {
                    let value = dive_trace::value_json::to_json(
                        state[0].as_ref().expect("x"),
                        options.eval.ctx,
                    )
                    .expect("encoded state");
                    serde_json::json!({"x": value})
                })
                .collect();
            let doc = TraceDoc::behavior(values)
                .expect("trace")
                .with_ending(Ending::Lasso {
                    back_to: candidate.back_to,
                })
                .with_properties(candidate.properties);
            assert_eq!(
                dive_trace::validate_doc(&model, &doc, &[]).0.validation,
                Validation::Valid,
                "{property}"
            );
        }
    }
}
