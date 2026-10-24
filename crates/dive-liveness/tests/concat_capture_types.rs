//! New primary words/counts are separate from the unchanged 19 capture cases.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "primary reference assertions"
)]

use std::{path::Path, sync::Arc};

use dive_check::CheckOptions;
use dive_liveness::analyze::{Analysis, Limits, analyze_final};
use dive_model::Model;
use dive_trace::{Ending, TraceDoc, Validation};
use serde_json::{Value, json};

fn bind(name: &str, options: &CheckOptions) -> Model {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/selectseq-concat-type-capability/specifications")
        .join(name);
    let source = dive_sema::analyze_file(
        &directory.join(format!("{name}.tla")),
        &dive_sema::Options::default(),
    )
    .expect("original source");
    assert!(source.accepted(), "{:?}", source.diagnostics());
    let config = dive_config::parse(
        &std::fs::read_to_string(directory.join(format!("{name}.cfg"))).expect("original config"),
    )
    .into_config()
    .expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&source).expect("IR")),
        &config,
        &dive_model::BindOptions {
            eval: options.eval,
            ..dive_model::BindOptions::tlc()
        },
    )
    .expect("bound model")
}

fn primary(name: &str) -> Value {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/golden/tlc-2026.10.06.014338/selectseq-concat-type-capability/specifications")
        .join(name)
        .join(format!("{name}.cfg.json"));
    serde_json::from_slice(&std::fs::read(path).expect("generated reference")).expect("JSON")
}

#[test]
fn captured_hold_fail_and_empty_callback_keep_primary_counts_with_cold_and_warm_evaluators() {
    for tlc in [true, false] {
        let options = if tlc {
            CheckOptions::tlc(0, 0)
        } else {
            CheckOptions::dive(0)
        };
        for name in [
            "CapturedConcatHolds",
            "CapturedConcatFails",
            "EmptySkipsCallbackError",
        ] {
            let expected = primary(name);
            let model = bind(name, &options);
            for iteration in 0..2 {
                let analysis =
                    analyze_final(&model, &options, Limits::default()).unwrap_or_else(|error| {
                        panic!("{name}/tlc={tlc}/iteration={iteration}: {error}")
                    });
                let counts = match &analysis {
                    Analysis::Holds(complete) => complete.stats,
                    Analysis::Candidate(candidate) => candidate.exploration.stats,
                };
                assert_eq!(
                    json!(counts.distinct),
                    expected["result"]["counts"]["distinct_states"],
                    "{name}"
                );
                assert_eq!(
                    json!(counts.generated),
                    expected["result"]["counts"]["states_generated"],
                    "{name}"
                );
                assert_eq!(
                    json!(counts.left_on_queue),
                    expected["result"]["counts"]["states_left_on_queue"],
                    "{name}"
                );
                assert_eq!(
                    json!(counts.depth),
                    expected["result"]["counts"]["depth"],
                    "{name}"
                );
                match analysis {
                    Analysis::Holds(_) => assert_eq!(expected["result"]["verdict"], "success"),
                    Analysis::Candidate(candidate) => {
                        assert_eq!(expected["result"]["verdict"], "liveness-violation");
                        assert_eq!(candidate.target_property, "P");
                        let states = candidate
                            .states
                            .iter()
                            .map(|state| {
                                let Some(dive_value::Value::Int(x)) = state[0] else {
                                    panic!("integer state")
                                };
                                json!({"x":x})
                            })
                            .collect();
                        let word = TraceDoc::behavior(states)
                            .expect("candidate word")
                            .with_ending(Ending::Lasso {
                                back_to: candidate.back_to,
                            })
                            .with_properties(candidate.properties);
                        assert_eq!(
                            dive_trace::validate_doc(&bind(name, &options), &word, &[])
                                .0
                                .validation,
                            Validation::Valid
                        );
                    }
                }
            }
            assert_eq!(model.take_output(), [] as [std::string::String; 0]);
        }
    }
}

#[test]
fn actual_pinned_stuttering_word_replays_independently_and_rejects_invalid_claims() {
    let expected = primary("CapturedConcatFails");
    for tlc in [true, false] {
        let options = if tlc {
            CheckOptions::tlc(0, 0)
        } else {
            CheckOptions::dive(0)
        };
        let word = TraceDoc::from_json(&expected["result"]["trace"]["json"])
            .expect("actual original TLC word")
            .with_ending(
                Ending::from_json(&expected["result"]["trace"]["ending"]).expect("actual ending"),
            )
            .with_properties(vec![
                expected["result"]["violated_property"]
                    .as_str()
                    .expect("property")
                    .to_owned(),
            ]);
        assert_eq!(
            dive_trace::validate_doc(&bind("CapturedConcatFails", &options), &word, &[])
                .0
                .validation,
            Validation::Valid
        );
        let mut bad_init = word.clone();
        bad_init.states[0].insert("x".to_owned(), json!(2));
        assert!(matches!(
            dive_trace::validate_doc(&bind("CapturedConcatFails", &options), &bad_init, &[])
                .0
                .validation,
            Validation::Invalid(_)
        ));
        assert!(
            matches!(
                dive_trace::validate_doc(&bind("CapturedConcatHolds", &options), &word, &[])
                    .0
                    .validation,
                Validation::Invalid(_)
            ),
            "a holding property cannot validate the same counterexample claim"
        );
    }
}

#[test]
fn string_only_native_name_creation_stays_a_named_refusal_after_the_type_change() {
    for tlc in [true, false] {
        let options = if tlc {
            CheckOptions::tlc(0, 0)
        } else {
            CheckOptions::dive(0)
        };
        let model = bind("StringConcatNames", &options);
        assert_eq!(primary("StringConcatNames")["result"]["verdict"], "success");
        let error = analyze_final(&model, &options, Limits::default())
            .expect_err("string creation remains conservative");
        let dive_liveness::analyze::AnalysisError::Planning(
            dive_liveness::plan::PlanningError::Unsupported(refusal),
        ) = error
        else {
            panic!("named planning refusal")
        };
        assert!(refusal.detail.contains("effect/provider/native"));
        assert_eq!(model.take_output(), [] as [std::string::String; 0]);
    }
}
