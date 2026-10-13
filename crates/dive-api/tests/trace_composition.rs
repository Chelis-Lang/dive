//! Feature-unified replay must never borrow the checker for composition.
#![allow(clippy::expect_used, reason = "test fixture construction")]

use std::sync::Arc;

use dive_api::trace::{SpecModel, TraceDoc, TraceOptions, Validation};
use dive_eval::{ActionComposition, St};
use dive_model::{Model, Pred};
use dive_trace::exists::{ExistsError, Limits, exists_successor};
use dive_trace::temporal::{TemporalError, evaluate_lasso};
use serde_json::json;

fn source(next: &str, extra: &str) -> String {
    format!(
        "---- MODULE Composition ----\nEXTENDS TLC\nVARIABLE x\nInit == x=0\nNext == {next}\n{extra}\n====\n"
    )
}

fn model(next: &str, extra: &str, mode: ActionComposition) -> Model {
    let src = source(next, extra);
    let analysis =
        dive_sema::analyze_source("Composition.tla", &src, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    let mut opts = dive_model::BindOptions::tlc();
    opts.eval.action_composition = mode;
    dive_model::bind(ir, &cfg, &opts).expect("model")
}

fn doc(x: i64) -> TraceDoc {
    TraceDoc::behavior(vec![json!({"x":0}), json!({"x":x})]).expect("document")
}

fn pred(m: &Model, name: &str) -> Pred {
    let d = m.ir.root_def(name).expect("definition");
    Pred::new(m.ir.def(d).body, m.evaluator.scope(d).expect("scope"))
}

#[test]
fn public_api_and_raw_replay_refuse_composition_for_valid_and_forged_words() {
    let src = source("(x'=1) \\cdot (x'=2)", "");
    let api = SpecModel::from_source(
        "Composition.tla",
        &src,
        "INIT Init\nNEXT Next\n",
        &TraceOptions::default(),
    )
    .expect("API");
    for x in [2, 9] {
        assert_eq!(
            api.validate(&doc(x)).validation,
            Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
        );
    }
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let m = model("(x'=1) \\cdot (x'=2)", "", mode);
        for x in [2, 9] {
            let trace = dive_trace::Trace::decode(&m, &doc(x)).expect("decode");
            assert_eq!(
                dive_trace::validate(&m, &trace).validation,
                Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
            );
        }
        // The ordinary evaluator is still permitted after the replay host drops.
        let d = m.ir.root_def("Next").expect("Next");
        for (x, expected) in [(2, true), (9, false)] {
            assert_eq!(
                m.evaluator
                    .eval_def(
                        d,
                        &St::action(Arc::from([Some(0.into())]), Arc::from([Some(x.into())]))
                    )
                    .expect("ordinary evaluation"),
                dive_value::Value::Bool(expected)
            );
        }
    }
}

#[test]
fn refused_composition_runs_no_operand_print_or_random_callback() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let m = model(
            "(PrintT(TRUE) /\\ x'=RandomElement({1})) \\cdot (x'=2)",
            "",
            mode,
        );
        assert_eq!(
            dive_trace::validate_doc(&m, &doc(2), &[]).0.validation,
            Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
        );
        assert!(!m.evaluator.has_print_event());
        assert_eq!(m.evaluator.take_output(), [] as [String; 0]);
    }
}

#[test]
fn swallowed_optional_label_composition_still_refuses_the_whole_word() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let m = model("x'=2", "Alias == (x'=1) \\cdot (x'=2)", mode);
        let mut document = doc(2);
        document.labels[1] = Some(dive_trace::Label {
            name: "Alias".to_owned(),
            location: None,
        });
        assert_eq!(
            dive_trace::validate_doc(&m, &document, &[]).0.validation,
            Validation::Unsupported(dive_trace::COMPOSITION_UNSUPPORTED.to_owned())
        );
    }
}

#[test]
fn untaken_composition_and_earlier_invalid_init_keep_real_results() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let m = model("IF TRUE THEN x'=2 ELSE ((x'=1) \\cdot (x'=2))", "", mode);
        assert_eq!(
            dive_trace::validate_doc(&m, &doc(2), &[]).0.validation,
            Validation::Valid
        );
        assert!(matches!(
            dive_trace::validate_doc(&m, &doc(9), &[]).0.validation,
            Validation::Invalid(_)
        ));
        let m = model("(x'=1) \\cdot (x'=2)", "", mode);
        let wrong_init =
            TraceDoc::behavior(vec![json!({"x":7}), json!({"x":2})]).expect("document");
        assert!(matches!(
            dive_trace::validate_doc(&m, &wrong_init, &[]).0.validation,
            Validation::Invalid(_)
        ));
    }
}

#[test]
fn public_finite_cover_and_lasso_predicates_refuse_shared_composition() {
    for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
        let m = model("x'=x", "A == (x'=1) \\cdot (x'=2)\nP == [][A]_x", mode);
        let state = Arc::from([Some(0.into())]);
        assert!(
            matches!(exists_successor(&m,&pred(&m,"A"),&state,None,Limits::default()),Err(ExistsError::Unsupported(ref feature)) if feature==dive_trace::COMPOSITION_UNSUPPORTED)
        );
        assert!(
            matches!(evaluate_lasso(&m,&pred(&m,"P"),&[state],0),Err(TemporalError::Unsupported(ref feature)) if feature==dive_trace::COMPOSITION_UNSUPPORTED)
        );
        assert!(!m.evaluator.has_print_event());
    }
}
