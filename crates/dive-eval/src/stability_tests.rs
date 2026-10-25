//! Adversarial facts for the optional producer proof, not a second evaluator.
#![allow(clippy::expect_used, clippy::panic, reason = "real evaluator fixtures")]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::*;
use crate::{Host, HostControl, HostError, Interrupted, Model, Options};
use dive_value::{Ctx, DeferredKind, DeferredSet, ValueError};

fn input(extra: &str) -> (Evaluator, dive_ir::DefId, dive_ir::DefId, dive_ir::DefId) {
    let source = format!(
        "---- MODULE Stable ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLE x\n{extra}\n====\n"
    );
    let analysis = dive_sema::analyze_source("Stable.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let init = ir.root_def("Init").expect("Init");
    let next = ir.root_def("Next").expect("Next");
    let pred = ir.root_def("P").expect("P");
    let evaluator = Evaluator::new(Model::new(ir), Options::tlc()).expect("evaluator");
    (evaluator, init, next, pred)
}

fn proof(
    evaluator: &Evaluator,
    init: dive_ir::DefId,
    next: dive_ir::DefId,
    pred: dive_ir::DefId,
) -> Option<ProducerStability> {
    let producer = |def| {
        (
            evaluator.ir().def(def).body,
            evaluator.scope(def).expect("scope"),
        )
    };
    evaluator.canonical_producer_stability(
        &[producer(init), producer(next)],
        &[producer(pred)],
        100_000,
    )
}

#[test]
fn canonical_record_and_tuple_producers_are_not_a_scalar_admission_rule() {
    for initial in ["[a |-> 0, b |-> <<1, 2>>]", "<<0, [a |-> 1]>>"] {
        let (ev, init, next, pred) = input(&format!(
            "Init == x = {initial}\nNext == UNCHANGED x\nP == x=x"
        ));
        let certificate = proof(&ev, init, next, pred).expect("general aggregate closure");
        assert!(certificate.belongs_to(&ev));
    }
}

#[test]
fn nonliteral_function_constructors_do_not_inherit_literal_normalization_authority() {
    let (ev, init, next, pred) = input("Init == x=(1 :> 0)\nNext == UNCHANGED x\nP == x=x");
    assert!(proof(&ev, init, next, pred).is_none());
}

#[test]
fn sortseq_uses_the_real_pure_callback_and_literal_tuple_result() {
    let (ev, init, next, pred) = input(
        "Init == x=0\nNext == UNCHANGED x\nLess(a,b) == a<b\nP == (SortSeq(<<1,0>>,Less) = <<0,1>>) /\\ (x=x)",
    );
    let before = ev
        .eval_def(pred, &St::state(Arc::from([Some(Value::Int(0))])))
        .expect("actual sorting callback");
    assert_eq!(before, Value::TRUE);
    assert!(proof(&ev, init, next, pred).is_some());
    assert_eq!(
        ev.eval_def(pred, &St::state(Arc::from([Some(Value::Int(0))])))
            .expect("same kernel after inspection"),
        Value::TRUE
    );
}

#[test]
fn enumeration_yield_fact_does_not_relabel_a_raw_domain_canonical() {
    let (ev, init, next, pred) =
        input("Init == x=0\nNext == IF x=0 THEN x' \\in {1,2} ELSE x'=0\nP == x>=0");
    assert!(proof(&ev, init, next, pred).is_some());
    let (raw, init, next, pred) = input("Init == x={0,1}\nNext == UNCHANGED x\nP == 0 \\in x");
    assert!(proof(&raw, init, next, pred).is_none());
}

#[test]
fn a_future_owned_filter_is_unknown_before_any_state_is_generated() {
    let (ev, init, next, pred) =
        input("Init == x=0\nNext == x'={p \\in ({0} \\X {0}): p[1]=0}\nP == x=x");
    assert!(proof(&ev, init, next, pred).is_none());
    assert_eq!(ev.take_output(), Vec::<String>::new());
}

#[test]
fn reordered_intermediate_raw_sets_are_not_hidden_by_canonical_state_values() {
    let (ev, init, next, pred) =
        input("Init == x=0\nNext == UNCHANGED x\nP == LET s == {x,1} IN x \\in s");
    assert!(proof(&ev, init, next, pred).is_none());
}

struct Probe(Arc<AtomicUsize>);
impl DeferredSet for Probe {
    fn kind(&self) -> DeferredKind {
        self.0.fetch_add(1, Ordering::SeqCst);
        DeferredKind::Filter
    }
    fn elements(&self, _ctx: Ctx) -> Result<Vec<Value>, ValueError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(vec![Value::Int(0)])
    }
    fn member(&self, _element: &Value, _ctx: Ctx) -> Option<Result<bool, ValueError>> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(Ok(true))
    }
    fn key(&self) -> Vec<Value> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Vec::new()
    }
    fn as_any(&self) -> Option<&dyn std::any::Any> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Some(self)
    }
}
impl std::fmt::Debug for Probe {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Probe")
    }
}

#[test]
fn opaque_providers_stay_unknown_even_after_a_real_normalization_warmup() {
    let (ev, _, _, _) = input("Init == x=0\nNext == UNCHANGED x\nP == TRUE");
    let calls = Arc::new(AtomicUsize::new(0));
    let value = Value::deferred(Arc::new(Probe(calls.clone())), Ctx::tlc());
    let mut guard = Guard {
        evaluator: &ev,
        work: 100_000,
        frames: BTreeMap::new(),
        abstracts: Vec::new(),
        active: Vec::new(),
    };
    let before = calls.load(Ordering::SeqCst);
    assert!(guard.value(&value, 0).is_none());
    assert_eq!(calls.load(Ordering::SeqCst), before);
    let Value::Set(set) = &value else {
        panic!("deferred set")
    };
    let _ = set.elements(Ctx::tlc()).expect("actual warmup");
    let warmed = calls.load(Ordering::SeqCst);
    assert!(warmed > before, "the provider observation must be genuine");
    assert!(guard.value(&value, 0).is_none());
    assert_eq!(calls.load(Ordering::SeqCst), warmed);
}

#[test]
fn a_warm_thunk_does_not_hide_its_original_effectful_capture() {
    let (ev, _, _, _) =
        input("Init == x=0\nNext == UNCHANGED x\nF(a) == a /\\ a\nP == F(PrintT(TRUE))");
    let source = ev.ir().root_def("P").expect("P");
    let def = ev.ir().root_def("F").expect("F");
    let Expr::Apply { args, .. } = ev.ir().expr(ev.ir().def(source).body) else {
        panic!("real Apply")
    };
    let scope = ev
        .scope_apply(&ev.scope(source).expect("source scope"), def, args)
        .expect("actual context");
    let body = ev.ir().def(def).body;
    assert_eq!(
        ev.eval_scoped(&scope, body, &St::empty())
            .expect("ordinary warmup"),
        Value::TRUE
    );
    let warmed = ev.take_output();
    assert_eq!(warmed.len(), 1, "real PrintT once through the thunk cache");
    assert!(
        ev.canonical_producer_stability(&[], &[(body, scope)], 100_000)
            .is_none()
    );
    assert_eq!(ev.take_output(), Vec::<String>::new());
}

#[test]
fn proof_work_and_foreign_scopes_decline_without_runtime_evaluation() {
    let (ev, init, next, pred) = input("Init == x=0\nNext == x'=1-x\nP == 1 \\div (x-x)=0");
    let root = (ev.ir().def(pred).body, ev.scope(pred).expect("scope"));
    assert!(ev.canonical_producer_stability(&[], &[root], 0).is_none());
    let foreign =
        Evaluator::new(Model::new(ev.ir().clone()), Options::tlc()).expect("independent Core");
    let root = (
        ev.ir().def(pred).body,
        foreign.scope(pred).expect("foreign scope"),
    );
    assert!(
        ev.canonical_producer_stability(&[], &[root], 100_000)
            .is_none()
    );
    assert!(
        proof(&ev, init, next, pred).is_some(),
        "failing primitive is inspected, not evaluated"
    );
    assert_eq!(ev.take_output(), Vec::<String>::new());
}

#[derive(Debug)]
struct Permission {
    calls: Arc<AtomicUsize>,
    deny: bool,
}
impl Host for Permission {
    fn check_native_access(&self, _op: StdOp) -> Result<(), HostError> {
        self.calls.fetch_add(1, Ordering::SeqCst);
        if self.deny {
            Err(HostError {
                code: 2182,
                message: "permission sentinel".into(),
            })
        } else {
            Ok(())
        }
    }
}
#[derive(Debug)]
struct Continue(Arc<AtomicUsize>);
impl HostControl for Continue {
    fn check(&self) -> Result<(), Interrupted> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn public_host_replacement_remains_visible_while_control_persists() {
    // Each evaluator-owned binder row charges loop_work; 2048 real rows
    // necessarily cross its 1024-row polling cadence. Literal domains and
    // TRUE bodies reach control before the only native call (x+1), even when
    // the replacement host denies every native operation.
    let domain = (0..2048)
        .map(|item| item.to_string())
        .collect::<Vec<_>>()
        .join(",");
    let (ev, _, _, pred) = input(&format!(
        "Init == x=0\nNext == UNCHANGED x\nP == IF (\\A item \\in {{{domain}}}: TRUE) THEN x+1 ELSE 0"
    ));
    let calls = Arc::new(AtomicUsize::new(0));
    let controls = Arc::new(AtomicUsize::new(0));
    ev.set_host(Arc::new(Permission {
        calls: calls.clone(),
        deny: false,
    }));
    let _scope = ev
        .host_control(Some(Arc::new(Continue(controls.clone()))))
        .expect("control");
    let state = St::state(Arc::from([Some(Value::Int(0))]));
    assert_eq!(
        ev.eval_def(pred, &state).expect("first host"),
        Value::Int(1)
    );
    let before = controls.load(Ordering::SeqCst);
    assert!(before > 0, "real binder work reached the active control");
    let _old = ev.replace_host(Some(Arc::new(Permission {
        calls: calls.clone(),
        deny: true,
    })));
    let error = ev
        .eval_def(pred, &state)
        .expect_err("replacement permission applies");
    assert!(error.to_string().contains("permission sentinel"));
    assert!(controls.load(Ordering::SeqCst) > before);
    assert_eq!(calls.load(Ordering::SeqCst), 2);
}
