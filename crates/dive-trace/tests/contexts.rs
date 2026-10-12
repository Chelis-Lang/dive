//! Structural integrity rejects malformed or foreign public contexts before replay.
#![allow(clippy::expect_used, reason = "fixture construction and assertions")]
use dive_model::{BindOptions, Model, Pred};
use dive_trace::{Ending, State, Trace, Validation};
use std::sync::Arc;

fn model(options: &BindOptions) -> Model {
    let src =
        "---- MODULE ContextTrace ----\nVARIABLE x\nInit == x=0\nNext == x'=x\nInv == x=0\n====\n";
    let a = dive_sema::analyze_source("ContextTrace.tla", src, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&a).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\nINVARIANT Inv\n")
        .into_config()
        .expect("cfg");
    dive_model::bind(ir, &cfg, options).expect("bound model")
}
fn pred(model: &Model, name: &str) -> Pred {
    let d = model.ir.root_def(name).expect(name);
    Pred::new(
        model.ir.def(d).body,
        model.evaluator.scope(d).expect("scope"),
    )
}
#[test]
fn foreign_scopes_and_replaced_model_arenas_never_certify() {
    for opts in [BindOptions::tlc(), BindOptions::dive()] {
        for metadata in [false, true] {
            let mut m = model(&opts);
            let foreign = model(&opts);
            let foreign_pred = pred(&foreign, "Inv");
            let current: State = Arc::from([Some(0.into())]);
            let states: Vec<State> = vec![current.clone()];
            let trace = Trace {
                states: states.clone(),
                labels: vec![None],
                ending: Ending::Behavior,
                length: Some(1),
                properties: vec![],
            };
            let next = pred(&m, "Next");
            let mut inv = pred(&m, "Inv");
            if metadata {
                m.ir = Arc::new((*m.ir).clone());
            } else {
                inv.scope = foreign_pred.scope;
                m.invariants[0].pred = inv.clone();
            }
            let names_before = m.names.len();
            assert!(matches!(
                dive_trace::temporal::evaluate_lasso(&m, &inv, &states, 0),
                Err(dive_trace::temporal::TemporalError::Unsupported(_))
            ));
            let exists = if metadata {
                next
            } else {
                Pred::new(
                    next.expr,
                    foreign
                        .evaluator
                        .scope(foreign.ir.root_def("Next").expect("Next"))
                        .expect("foreign scope"),
                )
            };
            assert!(matches!(
                dive_trace::exists::exists_successor(
                    &m,
                    &exists,
                    &current,
                    None,
                    dive_trace::exists::Limits::default()
                ),
                Err(dive_trace::exists::ExistsError::Unsupported(_))
            ));
            assert!(matches!(
                dive_trace::validate(&m, &trace, None).validation,
                Validation::Unsupported(_)
            ));
            let doc = dive_trace::TraceDoc::from_json(&serde_json::json!({
                "vars": ["x"], "counterexample": {"action": [], "state": [[1, {"x": 0}]]}
            }))
            .expect("TLC document")
            .with_ending(Ending::Behavior);
            assert!(matches!(
                dive_trace::validate_doc(&m, &doc, &[], None).0.validation,
                Validation::Unsupported(_)
            ));
            assert_eq!(
                m.names.len(),
                names_before,
                "no replay-time configuration interning"
            );
            assert_eq!(m.take_output(), Vec::<String>::new());
        }
    }
}
#[test]
fn admitted_clones_and_complete_words_remain_valid() {
    for opts in [BindOptions::tlc(), BindOptions::dive()] {
        let m = model(&opts);
        let states = vec![Arc::from([Some(0.into())])];
        let predicate = pred(&m, "Inv");
        assert_eq!(
            dive_trace::temporal::evaluate_lasso(&m, &predicate, &states, 0).expect("truth"),
            vec![true]
        );
        let next = pred(&m, "Next");
        assert!(matches!(
            dive_trace::exists::exists_successor(
                &m,
                &next,
                &states[0],
                None,
                dive_trace::exists::Limits::default()
            )
            .expect("existence"),
            dive_trace::exists::Decision::Enabled { .. }
        ));
        let trace = Trace {
            states,
            labels: vec![None],
            ending: Ending::Behavior,
            length: Some(1),
            properties: vec![],
        };
        assert_eq!(
            dive_trace::validate(&m, &trace, None).validation,
            Validation::Valid
        );
    }
}

#[test]
fn malformed_public_predicate_roots_and_locations_are_refused_before_replay() {
    for opts in [BindOptions::tlc(), BindOptions::dive()] {
        for role in ["init", "next", "invariant", "action-property"] {
            for location in [false, true] {
                for initial in [0, 1] {
                    let mut m = model(&opts);
                    if role == "action-property" {
                        m.implied_actions.push(m.invariants[0].clone());
                    }
                    let pred = match role {
                        "init" => &mut m.init[0].pred,
                        "next" => m.next.as_mut().expect("Next"),
                        "invariant" => &mut m.invariants[0].pred,
                        _ => &mut m.implied_actions[0].pred,
                    };
                    if location {
                        pred.location = dive_ir::ExprId(u32::MAX);
                    } else {
                        pred.expr = dive_ir::ExprId(u32::MAX);
                    }
                    let names_before = m.names.len();
                    let trace = Trace {
                        states: vec![Arc::from([Some(initial.into())])],
                        labels: vec![None],
                        ending: Ending::Behavior,
                        length: None,
                        properties: Vec::new(),
                    };
                    // A true Init must not hide malformed metadata; a false
                    // Init must not panic when rendering its source location.
                    assert!(matches!(
                        dive_trace::validate(&m, &trace, None).validation,
                        Validation::Unsupported(reason) if reason.contains("DIVE-EVAL-C001")
                    ));
                    for lossy in [false, true] {
                        let doc = if lossy {
                            dive_trace::TraceDoc::from_json(&serde_json::json!({
                                "vars": ["x"], "counterexample": {"action": [], "state": [[1, {"x": initial}]]}
                            }))
                            .expect("TLC document")
                            .with_ending(Ending::Behavior)
                        } else {
                            dive_trace::TraceDoc::behavior(vec![serde_json::json!({"x": initial})])
                                .expect("native document")
                        };
                        assert!(matches!(
                            dive_trace::validate_doc(&m, &doc, &[], None).0.validation,
                            Validation::Unsupported(reason) if reason.contains("DIVE-EVAL-C001")
                        ));
                    }
                    assert_eq!(m.names.len(), names_before);
                    assert_eq!(m.take_output(), Vec::<String>::new());
                }
            }
        }
    }
}
