//! Actual weak/strong fairness words, separate from initial invariant queries.
#![allow(clippy::expect_used, reason = "pinned primary fixtures")]

use std::{path::Path, sync::Arc};

use dive_model::{BindOptions, Pred};
use dive_trace::{Check, Ending, Trace, TraceDoc, Validation};
use serde_json::json;

#[test]
fn direct_and_instantiated_fairness_words_match_primary_observations() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/instance-fairness");
    let primary: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/instance-fairness/primary.json"))
            .expect("primary facts");
    let cases = primary["cases"].as_array().expect("cases");
    assert_eq!(cases.len(), 16);
    let mut failures = Vec::new();
    for case in cases {
        let id = case["id"].as_str().expect("id");
        let holds = case["holds"].as_bool().expect("observed property truth");
        assert_eq!(case["verdict_code"], if holds { 2193 } else { 2116 });
        let values = match case["scenario"].as_str().expect("scenario") {
            "continuous_starvation" => vec![(false, true)],
            "intermittent_starvation" => vec![(false, true), (false, false)],
            "disabled_stutter" => vec![(false, false)],
            "recurrent_closing" => vec![(false, true), (true, true)],
            other => unreachable_case(other),
        };
        for (profile, options) in [("tlc", BindOptions::tlc()), ("dive", BindOptions::dive())] {
            let source = fixtures.join(id);
            let analysis = dive_sema::analyze_file(
                &source.join("Fairness.tla"),
                &dive_sema::Options::default(),
            )
            .expect("original primary source");
            assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
            let config = dive_config::parse(
                &std::fs::read_to_string(source.join("Fairness.cfg")).expect("original config"),
            )
            .into_config()
            .expect("config");
            let model = dive_model::bind(
                Arc::new(dive_ir::lower(&analysis).expect("IR")),
                &config,
                &options,
            )
            .expect("model");
            let states = word_states(&model, &values);
            let def = model.ir.root_def("P").expect("P");
            let predicate = Pred::new(
                model.ir.def(def).body,
                model.evaluator.scope(def).expect("P scope"),
            );
            let truth = dive_trace::temporal::evaluate_lasso(&model, &predicate, &states, 0);
            if truth.as_ref().ok() != Some(&vec![holds; states.len()]) {
                failures.push(format!("{id}/{profile}/truth: {truth:?}"));
            }
            let trace = Trace {
                labels: vec![None; states.len()],
                length: Some(states.len()),
                states,
                ending: Ending::Lasso { back_to: 0 },
                properties: vec!["P".into()],
                concrete_closure: false,
                witnesses: Vec::new(),
                draws: Vec::new(),
            };
            let doc = trace.to_doc(&model).expect("serialize actual word");
            let transport = TraceDoc::parse(&doc.to_json().to_string()).expect("parse transport");
            for (route, result) in [
                ("typed", dive_trace::validate(&model, &trace).validation),
                (
                    "serialized",
                    dive_trace::validate_doc(&model, &transport, &config.model_values)
                        .0
                        .validation,
                ),
            ] {
                let expected = if holds {
                    matches!(&result, Validation::Invalid(f) if f.check == Check::Temporal)
                } else {
                    result == Validation::Valid
                };
                if !expected {
                    failures.push(format!("{id}/{profile}/{route}: {result:?}"));
                }
            }
            // A valid property violation cannot excuse a forged initial state.
            let mut forged = doc.clone();
            forged.states[0]["x"] = json!(true);
            let result = dive_trace::validate_doc(&model, &forged, &config.model_values)
                .0
                .validation;
            if !matches!(&result, Validation::Invalid(f) if f.check == Check::Init && f.state == 0)
            {
                failures.push(format!("{id}/{profile}/forged Init: {result:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

#[allow(clippy::panic, reason = "fixture schema invariant")]
fn unreachable_case(name: &str) -> Vec<(bool, bool)> {
    panic!("unknown scenario {name}")
}

#[allow(clippy::panic, reason = "fixture variable invariant")]
fn panic_variable(name: &str) -> dive_value::Value {
    panic!("unknown variable {name}")
}

fn word_states(model: &dive_model::Model, values: &[(bool, bool)]) -> Vec<dive_trace::State> {
    values
        .iter()
        .map(|(x, g)| {
            Arc::from(
                dive_trace::variables(model)
                    .iter()
                    .map(|name| {
                        Some(match name.as_str() {
                            "x" => (*x).into(),
                            "g" => (*g).into(),
                            other => panic_variable(other),
                        })
                    })
                    .collect::<Vec<_>>(),
            )
        })
        .collect()
}
