//! Interpretation heuristics must not change the predicates they certify.
#![allow(clippy::expect_used, reason = "test construction and assertions")]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use dive_eval::{Host, HostError};
use dive_model::Model;
use dive_trace::{Check, Ending, TraceDoc, Validation, validate_doc};
use dive_value::{Ctx, Value};
use serde_json::json;

fn model(init: &str) -> Model {
    let source = format!(
        "---- MODULE Effects ----\nEXTENDS Naturals, TLC\nVARIABLE x\nInit == {init}\nNext == x'=x\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("Effects.tla", &source, &dive_sema::Options::default());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    dive_model::bind(ir, &cfg, &dive_model::BindOptions::tlc()).expect("bind")
}

fn word(x: i64) -> TraceDoc {
    TraceDoc::from_json(&json!({
        "vars": ["x"], "counterexample": {"action": [], "state": [[1, {"x": x}]]}
    }))
    .expect("TLC document")
    .with_ending(Ending::Behavior)
}

#[test]
fn scoring_cannot_supply_a_register_value_that_makes_init_true() {
    let m = model("x=0 /\\ TLCGet(0)=1 /\\ TLCSet(0,1)");
    let report = validate_doc(&m, &word(0), &[], None).0;
    assert!(matches!(
        report.validation,
        Validation::Invalid(failure) if failure.check == Check::Init
    ));
}

#[test]
fn scoring_does_not_print_an_untaken_conjunct() {
    let m = model("x=0 /\\ PrintT(TRUE)");
    let report = validate_doc(&m, &word(1), &[], None).0;
    assert!(matches!(
        report.validation,
        Validation::Invalid(failure) if failure.check == Check::Init
    ));
    assert_eq!(m.take_output(), Vec::<String>::new());
}

#[derive(Debug, Default)]
struct DrawHost(AtomicUsize);

impl Host for DrawHost {
    fn random(&self, _: dive_ir::StdOp, _: &[Value], _: Ctx) -> Option<Result<Value, HostError>> {
        self.0.fetch_add(1, Ordering::Relaxed);
        Some(Ok(Value::Int(0)))
    }
}

#[test]
fn scoring_never_draws_from_the_inherited_host() {
    for (init, x, draws) in [
        ("x=0 /\\ RandomElement({0})=0", 1, false),
        ("x=RandomElement({0})", 0, true),
    ] {
        let m = model(init);
        let host = Arc::new(DrawHost::default());
        m.evaluator.set_host(host.clone());
        let report = validate_doc(&m, &word(x), &[], None).0;
        if draws {
            assert!(matches!(
                report.validation,
                Validation::Unsupported(reason) if reason.contains(dive_trace::RANDOM_UNSUPPORTED)
            ));
        } else {
            assert!(matches!(report.validation, Validation::Invalid(_)));
        }
        assert_eq!(host.0.load(Ordering::Relaxed), 0);
    }
}
