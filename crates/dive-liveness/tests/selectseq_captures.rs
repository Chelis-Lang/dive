//! Pinned source captures retain original evaluation and independent replay.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use std::{path::Path, sync::Arc};

use dive_liveness::analyze::{Analysis, Limits, analyze_final};
use dive_model::Model;
use dive_runtime::CheckOptions;
use dive_trace::{Ending, TraceDoc, Validation};
use serde_json::{Value, json};

fn bind(directory: &Path, options: &CheckOptions) -> Model {
    let source = dive_sema::analyze_file(
        &directory.join("CallbackCapture.tla"),
        &dive_sema::Options::default(),
    )
    .expect("source");
    assert!(source.accepted(), "{:?}", source.diagnostics());
    let config = dive_config::parse(
        &std::fs::read_to_string(directory.join("CallbackCapture.cfg")).expect("config"),
    )
    .into_config()
    .expect("parsed config");
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

#[test]
fn complete_primary_selection_preserves_counts_truth_and_fresh_replay() {
    let repository = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let corpus = repository.join("conformance/corpus/selectseq-captures/specifications");
    let goldens = repository
        .join("conformance/golden/tlc-2026.10.06.014338/selectseq-captures/specifications");
    let mut directories = std::fs::read_dir(&corpus)
        .expect("complete corpus")
        .map(|entry| entry.expect("entry").path())
        .collect::<Vec<_>>();
    directories.sort();
    assert_eq!(directories.len(), 19);
    for tlc in [true, false] {
        let options = if tlc {
            CheckOptions::tlc(0, 0)
        } else {
            CheckOptions::dive(0)
        };
        for directory in &directories {
            let name = directory.file_name().expect("name").to_str().expect("utf8");
            let primary: Value = serde_json::from_slice(
                &std::fs::read(goldens.join(name).join("CallbackCapture.cfg.json"))
                    .expect("primary"),
            )
            .expect("primary JSON");
            let model = bind(directory, &options);
            let refuses = matches!(
                name,
                "empty_effect_capture_holds"
                    | "configured_action_capture_holds"
                    | "unused_effect_actual_holds"
            );
            // The second run reuses evaluator definition/lazy-value caches.
            for iteration in 0..2 {
                let analysis = analyze_final(&model, &options, Limits::default());
                if refuses {
                    let error = analysis.expect_err("unsafe capture must refuse");
                    let dive_liveness::analyze::AnalysisError::Planning(
                        dive_liveness::plan::PlanningError::Unsupported(refusal),
                    ) = error
                    else {
                        panic!("{name}: expected named planning refusal: {error}")
                    };
                    assert!(
                        refusal.detail.contains("effect") || refusal.detail.contains("primed"),
                        "{name}: {}",
                        refusal.detail
                    );
                    continue;
                }
                let analysis = analysis.unwrap_or_else(|error| {
                    panic!("{name}/tlc={tlc}/iteration{iteration}: {error}")
                });
                let counts = match &analysis {
                    Analysis::Holds(complete) => complete.stats,
                    Analysis::Candidate(candidate) => candidate.exploration.stats,
                };
                assert_eq!(
                    json!(counts.distinct),
                    primary["result"]["counts"]["distinct_states"],
                    "{name}"
                );
                assert_eq!(
                    json!(counts.generated),
                    primary["result"]["counts"]["states_generated"],
                    "{name}"
                );
                assert_eq!(
                    json!(counts.depth),
                    primary["result"]["counts"]["depth"],
                    "{name}"
                );
                match analysis {
                    Analysis::Holds(_) => {
                        assert_eq!(primary["result"]["verdict"], "success", "{name}");
                    }
                    Analysis::Candidate(candidate) => {
                        assert_eq!(primary["result"]["verdict"], "liveness-violation", "{name}");
                        verify_candidate(directory, &options, candidate);
                    }
                }
            }
            assert_eq!(
                model.take_output(),
                Vec::<String>::new(),
                "{name}: no admission effects"
            );
        }
    }
}

fn verify_candidate(
    directory: &Path,
    options: &CheckOptions,
    candidate: dive_liveness::analyze::Candidate,
) {
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
    let doc = TraceDoc::behavior(states)
        .expect("word")
        .with_ending(Ending::Lasso {
            back_to: candidate.back_to,
        })
        .with_properties(candidate.properties);
    let fresh = bind(directory, options);
    assert_eq!(
        dive_trace::validate_doc(&fresh, &doc, &[]).0.validation,
        Validation::Valid,
        "{}",
        directory.display()
    );
}
