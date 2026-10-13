//! Private startup must retain independent composition refusal and cache policy.
#![allow(clippy::expect_used, reason = "test fixture construction")]

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_eval::{ActionComposition, Options, St};
use dive_model::Recipe;
use dive_trace::Session;
use serde_json::json;
use std::sync::Arc;

fn source(next: &str, extra: &str) -> String {
    format!(
        "---- MODULE PrivateComposition ----\nEXTENDS TLC\nVARIABLE x\n{extra}\nASSUME TLCSet(1,42)\nInit == x=0 /\\ TLCGet(1)=42\nNext == {next}\n====\n"
    )
}

fn build_session(next: &str, extra: &str, mode: ActionComposition) -> Session {
    let source = source(next, extra);
    let analysis = dive_sema::analyze_source(
        "PrivateComposition.tla",
        &source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let config = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    let mut options = Options::tlc();
    options.action_composition = mode;
    Session::new(Recipe::new(ir, config, options, false), None)
}

fn doc(x: i64, lossy: bool) -> TraceDoc {
    if lossy {
        TraceDoc::from_json(
            &json!({"vars":["x"],"counterexample":{"action":[],"state":[[1,{"x":0}],[2,{"x":x}]]}}),
        )
        .expect("TLC document")
        .with_ending(dive_trace::Ending::Behavior)
    } else {
        TraceDoc::behavior(vec![json!({"x":0}), json!({"x":x})]).expect("document")
    }
}

#[test]
fn private_startup_refuses_reached_composition_in_both_modes_and_readers() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let session = build_session("W((x'=1) \\cdot (x'=2))", "W(p) == p", mode);
        for lossy in [false, true] {
            for x in [2, 9] {
                for _ in 0..2 {
                    assert_eq!(
                        session.read(&doc(x, lossy)).0.validation,
                        Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
                    );
                }
            }
        }
        let session = build_session("x'=2", "Alias == (x'=1) \\cdot (x'=2)", mode);
        let mut document = doc(2, false);
        document.labels[1] = Some(dive_trace::Label {
            name: "Alias".to_owned(),
            location: None,
        });
        assert_eq!(
            session.read(&document).0.validation,
            Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
        );
    }
}

#[test]
fn private_startup_keeps_untaken_composition_and_wrong_init_decidable() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let session = build_session("IF TRUE THEN x'=2 ELSE ((x'=1) \\cdot (x'=2))", "", mode);
        for lossy in [false, true] {
            assert_eq!(session.read(&doc(2, lossy)).0.validation, Validation::Valid);
            assert!(matches!(
                session.read(&doc(9, lossy)).0.validation,
                Validation::Invalid(_)
            ));
        }
        let session = build_session("(x'=1) \\cdot (x'=2)", "", mode);
        let wrong = TraceDoc::behavior(vec![json!({"x":7}), json!({"x":2})]).expect("document");
        assert!(matches!(
            session.read(&wrong).0.validation,
            Validation::Invalid(_)
        ));
    }
}

#[test]
fn caller_warmed_composition_cannot_authorize_private_api_validation() {
    let source = source("W((x'=1) \\cdot (x'=2))", "W(p) == p");
    let api = SpecModel::from_source(
        "PrivateComposition.tla",
        &source,
        "INIT Init\nNEXT Next\n",
        &TraceOptions::default(),
    )
    .expect("API");
    let model = api.model();
    let next = model.ir.root_def("Next").expect("Next");
    for x in [2, 9] {
        let state = St::action(Arc::from([Some(0.into())]), Arc::from([Some(x.into())]));
        assert_eq!(
            model.evaluator.eval_def(next, &state).expect("caller warm"),
            dive_value::Value::Bool(x == 2)
        );
        for lossy in [false, true] {
            assert_eq!(
                api.validate(&doc(x, lossy)).validation,
                Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
            );
        }
        assert_eq!(
            model
                .evaluator
                .eval_def(next, &state)
                .expect("ordinary caller remains usable"),
            dive_value::Value::Bool(x == 2)
        );
    }
}
