//! Independent complete-word checks for actual constant declaration fallbacks.
#![allow(clippy::expect_used, reason = "original primary fixture assertions")]

use dive_model::{BindOptions, Pred};
use dive_trace::{Check, Ending, Trace, TraceDoc, Validation};
use std::{path::Path, sync::Arc};

#[test]
fn constant_fallback_words_check_behavior_and_reject_false_property_claims() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/instance-constants");
    let facts: serde_json::Value =
        serde_json::from_str(include_str!("fixtures/instance-constants/primary.json"))
            .expect("primary facts");
    let cases = facts["cases"].as_array().expect("four cases");
    assert_eq!(cases.len(), 4);
    let mut failures = Vec::new();
    for case in cases {
        let id = case["id"].as_str().expect("case");
        for (profile, options) in [("tlc", BindOptions::tlc()), ("dive", BindOptions::dive())] {
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
            assert_eq!(dive_trace::variables(&model), ["x"]);
            let states: Vec<dive_trace::State> = if id == "constant_bound_instance" {
                vec![false]
            } else {
                vec![false, true]
            }
            .into_iter()
            .map(|x| Arc::from([Some(x.into())]))
            .collect();
            let def = model.ir.root_def("P").expect("property");
            let property = Pred::new(
                model.ir.def(def).body,
                model.evaluator.scope(def).expect("scope"),
            );
            for (name, pred) in std::iter::once(("P", &property))
                .chain(model.temporals.iter().map(|b| (b.name.as_str(), &b.pred)))
            {
                let truth = dive_trace::temporal::evaluate_lasso(&model, pred, &states, 0);
                if truth.as_ref().ok() != Some(&vec![true; states.len()]) {
                    failures.push(format!("{id}/{profile}/{name}: {truth:?}"));
                }
            }
            let trace = Trace {
                labels: vec![None; states.len()],
                length: Some(states.len()),
                states,
                ending: Ending::Lasso { back_to: 0 },
                properties: vec!["P".into()],
                concrete_closure: false,
                witnesses: Vec::new(),
            };
            let doc = trace.to_doc(&model).expect("document");
            let parsed = TraceDoc::parse(&doc.to_json().to_string()).expect("transport");
            for (route, result) in [
                ("typed", dive_trace::validate(&model, &trace).validation),
                (
                    "serialized",
                    dive_trace::validate_doc(&model, &parsed, &config.model_values)
                        .0
                        .validation,
                ),
            ] {
                if !matches!(&result,Validation::Invalid(f) if f.check==Check::Temporal) {
                    failures.push(format!("{id}/{profile}/{route}: {result:?}"));
                }
            }
            let mut forged = doc;
            forged.states[0]["x"] = serde_json::json!(true);
            let result = dive_trace::validate_doc(&model, &forged, &config.model_values)
                .0
                .validation;
            if !matches!(&result,Validation::Invalid(f) if f.check==Check::Init && f.state==0) {
                failures.push(format!("{id}/{profile}/forged Init: {result:?}"));
            }
        }
    }
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
