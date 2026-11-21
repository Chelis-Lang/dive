//! Genuine periodic violations retain original constant declaration fallbacks.
#![allow(clippy::expect_used, reason = "pinned primary fixture assertions")]

use dive_model::{BindOptions, Pred};
use dive_trace::{Check, Ending, Trace, TraceDoc, Validation};
use std::{path::Path, sync::Arc};

#[test]
fn absent_constants_validate_genuine_words_and_reject_forged_edges() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/instance-constants");
    let facts: serde_json::Value = serde_json::from_str(include_str!(
        "fixtures/instance-constants/primary-violations.json"
    ))
    .expect("primary facts");
    let cases = facts["cases"].as_array().expect("two cases");
    assert_eq!(cases.len(), 2);
    for case in cases {
        let id = case["id"].as_str().expect("case");
        for options in [BindOptions::tlc(), BindOptions::dive()] {
            let path = fixtures.join(id);
            let source =
                dive_sema::analyze_file(&path.join("Constant.tla"), &dive_sema::Options::default())
                    .expect("source");
            assert!(source.accepted(), "{:?}", source.diagnostics());
            let config = dive_config::parse(
                &std::fs::read_to_string(path.join("Constant.cfg")).expect("config"),
            )
            .into_config()
            .expect("parsed config");
            let model = dive_model::bind(
                Arc::new(dive_ir::lower(&source).expect("IR")),
                &config,
                &options,
            )
            .expect("model");
            let states: Vec<dive_trace::State> = [false, true]
                .into_iter()
                .map(|x| Arc::from([Some(x.into())]))
                .collect();
            for (name, expected) in [("P", true), ("Bad", false)] {
                let def = model.ir.root_def(name).expect("property");
                let pred = Pred::new(
                    model.ir.def(def).body,
                    model.evaluator.scope(def).expect("scope"),
                );
                assert_eq!(
                    dive_trace::temporal::evaluate_lasso(&model, &pred, &states, 0)
                        .expect("exact opposite truths"),
                    vec![expected; 2],
                    "{id}/{name}"
                );
            }
            let trace = Trace {
                labels: vec![None; states.len()],
                length: Some(states.len()),
                states,
                ending: Ending::Lasso { back_to: 0 },
                properties: vec!["Bad".into()],
                concrete_closure: false,
                witnesses: Vec::new(),
            };
            for _ in 0..2 {
                assert_eq!(
                    dive_trace::validate(&model, &trace).validation,
                    Validation::Valid
                );
                let doc = trace.to_doc(&model).expect("document");
                let parsed = TraceDoc::parse(&doc.to_json().to_string()).expect("transport");
                assert_eq!(
                    dive_trace::validate_doc(&model, &parsed, &config.model_values)
                        .0
                        .validation,
                    Validation::Valid,
                    "{id}"
                );
                let mut init = doc.clone();
                init.states[0]["x"] = serde_json::json!(true);
                assert!(matches!(
                    dive_trace::validate_doc(&model, &init, &config.model_values).0.validation,
                    Validation::Invalid(ref failure) if failure.check == Check::Init && failure.state == 0
                ));
                let mut next = doc;
                next.states[1]["x"] = serde_json::json!(2);
                assert!(matches!(
                    dive_trace::validate_doc(&model, &next, &config.model_values).0.validation,
                    Validation::Invalid(ref failure) if failure.check == Check::Next && failure.state == 1
                ));
            }
        }
    }
}
