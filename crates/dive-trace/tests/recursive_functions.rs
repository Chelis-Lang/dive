//! ENABLED over an actual recursive permutation domain.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "bounded independent controls"
)]
use dive_model::{Model, Pred};
use dive_trace::{State, enabled::enabled};
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
fn actual_permutation_domain_enabledness() {
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
            let result = enabled(&model, &action, Some(vars.expr), &state).expect("decision");
            assert_eq!(result, !(elements.is_empty() && done));
        }
    }
}
