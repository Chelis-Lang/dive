//! Independent recursive finite domains cannot invent covers or hide primes.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded independent controls"
)]
use dive_model::{Model, Pred};
use dive_trace::{
    State,
    exists::{Decision, Limits, exists_successor_pure},
};
use dive_value::{Ctx, Value};
use std::{path::PathBuf, sync::Arc};
fn pred(model: &Model, name: &str) -> Pred {
    let d = model.ir.root_def(name).expect(name);
    Pred::new(
        model.ir.def(d).body,
        model.evaluator.scope(d).expect("scope"),
    )
}
fn bound(source: &str, config: &str) -> Model {
    let a = dive_sema::analyze_source("RecursiveCover.tla", source, &dive_sema::Options::default());
    assert!(a.accepted(), "{:?}", a.diagnostics());
    let cfg = dive_config::parse(config).into_config().expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&a).expect("IR")),
        &cfg,
        &dive_model::BindOptions::tlc(),
    )
    .expect("model")
}
#[test]
fn actual_permutation_domain_enabledness_uses_original_relation_and_complete_covers() {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/recursive-function/PermSeqsScope.tla");
    let source = std::fs::read_to_string(path)
        .expect("actual fixture")
        .replace("MODULE PermSeqsScope", "MODULE RecursiveCover");
    let model = bound(&source, "CONSTANT N=2\nINIT Init\nNEXT Next\n");
    for elements in [vec![], vec![1], vec![1, 2]] {
        for done in [false, true] {
            let pending = Value::set_raw(
                elements.iter().copied().map(Value::Int).collect(),
                Ctx::tlc(),
            );
            let state: State = Arc::from([Some(pending), Some(Value::Bool(done))]);
            let action = pred(&model, "Schedule");
            let vars = pred(&model, "vars");
            let result = exists_successor_pure(
                &model,
                &action,
                &state,
                Some(&vars),
                Limits::default(),
                4096,
            )
            .expect("finite decision");
            let expected = !(elements.is_empty() && done);
            assert_eq!(matches!(result, Decision::Enabled { .. }), expected);
            if let Decision::Enabled { witness, .. } = result {
                assert_eq!(
                    model
                        .eval(&action, &dive_eval::St::action(state, witness))
                        .expect("original witness"),
                    Value::Bool(true)
                );
            }
        }
    }
}
#[test]
fn empty_recursive_domains_and_captured_or_outer_primes_never_certify_disabled() {
    let source = "---- MODULE RecursiveCover ----\nEXTENDS Naturals\nVARIABLES x,y\nInit==x=0 /\\ y=0\nNext==UNCHANGED <<x,y>>\nR(S)==LET f[t \\in {}]==IF t=0 THEN S ELSE f[t] IN DOMAIN f\nP(S)==LET f[t \\in S]==IF t=0 THEN {0} ELSE f[t] IN DOMAIN f\nA==x' \\in R(y') /\\ y'=0\nB==LET captured==y' IN x' \\in R(captured) /\\ y'=0\nC==x' \\in P({y})' /\\ y'=0\nD==x' \\in (LET f[t \\in {}]==IF t=0 THEN y' ELSE f[t] IN DOMAIN f) /\\ y'=0\n====\n";
    let model = bound(source, "INIT Init\nNEXT Next\n");
    let before: State = Arc::from([Some(0.into()), Some(0.into())]);
    for name in ["A", "B", "C", "D"] {
        assert!(
            exists_successor_pure(
                &model,
                &pred(&model, name),
                &before,
                None,
                Limits::default(),
                4096,
            )
            .is_err(),
            "{name}: empty domains cannot prove Disabled"
        );
    }
}
#[test]
fn recursive_values_do_not_supply_missing_successor_covers_or_bypass_budgets() {
    let source = "---- MODULE RecursiveCover ----\nEXTENDS Naturals\nVARIABLES x,y\nInit==x=0 /\\ y=0\nNext==UNCHANGED <<x,y>>\nR(S)==LET f[t \\in S]==IF t=0 THEN {0} ELSE f[t-1] IN f[0]\nA==x' \\in R({0}) /\\ y'>0\nB==x' \\in R({0}) /\\ y'=0\n====\n";
    let model = bound(source, "INIT Init\nNEXT Next\n");
    let before: State = Arc::from([Some(0.into()), Some(0.into())]);
    assert!(
        exists_successor_pure(
            &model,
            &pred(&model, "A"),
            &before,
            None,
            Limits::default(),
            4096
        )
        .is_err()
    );
    assert!(matches!(
        exists_successor_pure(
            &model,
            &pred(&model, "B"),
            &before,
            None,
            Limits::default(),
            4096
        )
        .expect("covered relation"),
        Decision::Enabled { .. }
    ));
    for limits in [
        Limits {
            expressions: 0,
            ..Limits::default()
        },
        Limits {
            domain_values: 0,
            ..Limits::default()
        },
        Limits {
            assignments: 0,
            ..Limits::default()
        },
    ] {
        assert!(
            exists_successor_pure(&model, &pred(&model, "B"), &before, None, limits, 4096).is_err()
        );
    }
}
