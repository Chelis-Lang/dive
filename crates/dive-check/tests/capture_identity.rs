//! Replay identity preparation shares the producer kernel and no host authority.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "actual capture and replay assertions"
)]
use dive_check::{CheckOptions, capture};
use dive_eval::{ErrorClass, next::StateVals};
use dive_value::{Value, codec};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

fn model(source: &str, opts: &CheckOptions) -> dive_model::Model {
    let a = dive_sema::analyze_source("Identity.tla", source, &dive_sema::Options::default());
    assert!(
        a.accepted(),
        "{:?} {:?}",
        a.diagnostics(),
        a.syntax_diagnostics()
    );
    let cfg = dive_config::parse("INIT Init\nNEXT Next\n")
        .into_config()
        .expect("config");
    dive_model::bind(
        Arc::new(dive_ir::lower(&a).expect("IR")),
        &cfg,
        &dive_model::BindOptions {
            eval: opts.eval,
            no_deadlock: false,
            host: None,
        },
    )
    .expect("model")
}
fn encoded(state: &StateVals, opts: &CheckOptions) -> Vec<u8> {
    let mut out = Vec::new();
    for value in state.iter().flatten() {
        codec::encode(value, opts.eval.ctx, &mut out).expect("lossless");
    }
    out
}

#[test]
fn exact_and_fingerprint_replay_match_real_producer_normalization_without_flag_repair() {
    let source = "---- MODULE Identity ----\nEXTENDS Naturals\nVARIABLE x\nInit == x = <<1..2,[i \\in {\"a\"}|->1]>>\nNext == x' = <<{2,1},[a|->1]>>\n====\n";
    for base in [CheckOptions::dive(7), CheckOptions::tlc(3, 7)] {
        for store in [
            dive_store::StoreMode::Exact,
            dive_store::StoreMode::Fingerprint,
        ] {
            let opts = CheckOptions {
                store,
                ..base.clone()
            };
            let m = model(source, &opts);
            let capture::Exploration::Complete(c) =
                capture::explore_with_evidence(&m, &opts, capture::Limits::default())
                    .expect("capture")
            else {
                panic!("capture")
            };
            let mut replay = None;
            m.evaluator
                .successors(&m.actions, &c.graph.states[0], &mut |s| {
                    replay = Some(s.state.into());
                    dive_eval::next::Flow::Stop
                })
                .expect("successor");
            let replay = replay.expect("original replay");
            let before = c.stats;
            let charge = capture::prepare_replay_identity(&m, &c.options, &replay, 100_000)
                .expect("actual identity");
            assert!(charge > 0 && charge <= 100_000);
            assert_eq!(
                encoded(&replay, &opts),
                encoded(&c.graph.concrete[0][0].state, &opts)
            );
            assert_eq!(c.stats, before);
            let Some(Value::Fcn(outer)) = &replay[0] else {
                panic!("tuple")
            };
            assert!(outer.is_tuple_literal());
            let Some(Value::Fcn(record)) = outer.as_tuple().and_then(|e| e.get(1)) else {
                panic!("record")
            };
            assert!(record.is_record_literal());
            let Some(Value::Fcn(initial)) = &c.graph.states[0][0] else {
                panic!("tuple")
            };
            let Some(Value::Fcn(function)) = initial.as_tuple().and_then(|e| e.get(1)) else {
                panic!("function")
            };
            assert!(!function.is_record_literal());
            assert_eq!(m.take_output(), [] as [String; 0]);
            assert_eq!(
                capture::prepare_replay_identity(&m, &c.options, &replay, 1)
                    .expect_err("bounded")
                    .class(),
                ErrorClass::ResourceLimit
            );
        }
    }
}

#[test]
fn preparation_preserves_shared_typed_key_errors_and_refuses_foreign_profile() {
    let source = "---- MODULE Identity ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=Nat\n====\n";
    let opts = CheckOptions::tlc(3, 7);
    let m = model(source, &opts);
    let mut replay = None;
    m.evaluator
        .successors(&m.actions, &Arc::from([Some(Value::Int(0))]), &mut |s| {
            replay = Some(s.state.into());
            dive_eval::next::Flow::Stop
        })
        .expect("original");
    let replay = replay.expect("state");
    let error =
        capture::prepare_replay_identity(&m, &opts, &replay, 1000).expect_err("real FP key error");
    assert_eq!(error.class(), ErrorClass::Eval);
    assert_eq!(error.ec().code(), 1000);
    let capture::Exploration::Stopped(raw) =
        capture::explore_with_evidence(&m, &opts, capture::Limits::default()).expect("raw capture")
    else {
        panic!("raw error")
    };
    assert!(
        matches!(&raw.verdict,dive_check::Verdict::EvaluationError{site:dive_check::ErrorSite::Fingerprint,code:2103,reported:1000,cause,..} if cause.class()==error.class() && cause.ec()==error.ec() && cause.message()==error.message()),
        "{:?} {error}",
        raw.verdict
    );
    assert_eq!(
        capture::prepare_replay_identity(&m, &CheckOptions::dive(7), &replay, 1000)
            .expect_err("foreign profile")
            .class(),
        ErrorClass::Unsupported
    );
}

#[derive(Debug)]
struct Deferred(Arc<AtomicUsize>);
impl dive_value::DeferredSet for Deferred {
    fn kind(&self) -> dive_value::DeferredKind {
        dive_value::DeferredKind::Filter
    }
    fn elements(&self, _: dive_value::Ctx) -> dive_value::Result<Vec<Value>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![Value::Int(1)])
    }
    fn member(&self, _: &Value, _: dive_value::Ctx) -> Option<dive_value::Result<bool>> {
        None
    }
}
#[test]
fn bounded_preparation_does_not_run_opaque_value_callbacks() {
    let opts = CheckOptions::dive(7);
    let m = model(
        "---- MODULE Identity ----\nEXTENDS Naturals\nVARIABLE x\nInit == x=0\nNext == x'=x\n====\n",
        &opts,
    );
    let calls = Arc::new(AtomicUsize::new(0));
    let state = Arc::from([Some(Value::tuple(vec![Value::deferred(
        Arc::new(Deferred(calls.clone())),
        opts.eval.ctx,
    )]))]);
    let before = calls.load(Ordering::SeqCst);
    assert_eq!(
        capture::prepare_replay_identity(&m, &opts, &state, 10000)
            .expect_err("opaque callback")
            .class(),
        ErrorClass::Unsupported
    );
    assert_eq!(calls.load(Ordering::SeqCst), before);
}
