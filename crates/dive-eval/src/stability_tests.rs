//! Adversarial facts for the optional producer proof, not a second evaluator.
#![allow(clippy::expect_used, clippy::panic, reason = "real evaluator fixtures")]

use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use super::ProducerStability;
use crate::env::Sv;
use crate::{Evaluator, St};
use crate::{Host, HostControl, HostError, Interrupted, Model, Options};
use dive_ir::{Expr, StdOp};
use dive_value::Value;
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
    evaluator
        .canonical_producer_stability_budgeted(
            &[producer(init), producer(next)],
            &[producer(pred)],
            None,
        )
        .expect("inspection without a run interruption")
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
    let (ev, _, _, pred) = input("Init == x=0\nNext == UNCHANGED x\nP(a) == a=a");
    let calls = Arc::new(AtomicUsize::new(0));
    let value = Value::deferred(Arc::new(Probe(calls.clone())), Ctx::tlc());
    let mut scope = ev.scope(pred).expect("actual predicate scope");
    scope.env.bind(0, Sv::Val(value.clone()));
    let roots = [(ev.ir().def(pred).body, scope)];
    let before = calls.load(Ordering::SeqCst);
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &roots, None)
            .expect("opaque storage is a semantic refusal")
            .is_none()
    );
    assert_eq!(calls.load(Ordering::SeqCst), before);
    let Value::Set(set) = &value else {
        panic!("deferred set")
    };
    let _ = set.elements(Ctx::tlc()).expect("actual warmup");
    let warmed = calls.load(Ordering::SeqCst);
    assert!(warmed > before, "the provider observation must be genuine");
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &roots, None)
            .expect("opaque storage is a semantic refusal")
            .is_none()
    );
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
        ev.canonical_producer_stability_budgeted(&[], &[(body, scope)], None)
            .expect("effectful capture is refused without evaluation")
            .is_none()
    );
    assert_eq!(ev.take_output(), Vec::<String>::new());
}

#[test]
fn run_budget_and_foreign_scopes_stop_without_runtime_evaluation() {
    let (ev, init, next, pred) = input("Init == x=0\nNext == x'=1-x\nP == 1 \\div (x-x)=0");
    let root = (ev.ir().def(pred).body, ev.scope(pred).expect("scope"));
    let control = StopControl(AtomicUsize::new(0));
    let error = ev
        .canonical_producer_stability_budgeted(&[], &[root], Some(&control))
        .expect_err("the actual run control must stop proof work");
    assert_eq!(error.class(), crate::ErrorClass::Interrupted);
    assert_eq!(error.interruption(), Some(Interrupted::new(727)));
    assert_eq!(control.0.load(Ordering::SeqCst), 1);
    let foreign =
        Evaluator::new(Model::new(ev.ir().clone()), Options::tlc()).expect("independent Core");
    let root = (
        ev.ir().def(pred).body,
        foreign.scope(pred).expect("foreign scope"),
    );
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &[root], None)
            .expect("foreign scope is a semantic refusal")
            .is_none()
    );
    assert!(
        proof(&ev, init, next, pred).is_some(),
        "failing primitive is inspected, not evaluated"
    );
    assert_eq!(ev.take_output(), Vec::<String>::new());
}

#[derive(Debug)]
struct StopControl(AtomicUsize);
impl HostControl for StopControl {
    fn check(&self) -> Result<(), Interrupted> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Err(Interrupted::new(727))
    }
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
    let (ev, init, next, pred) = input(&format!(
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
    let producers = [init, next].map(|def| {
        (
            ev.ir().def(def).body,
            ev.scope(def).expect("actual producer scope"),
        )
    });
    assert!(
        ev.canonical_producer_stability_budgeted(&producers, &[], None)
            .expect("installed control permits metadata inspection")
            .is_some()
    );
    assert!(controls.load(Ordering::SeqCst) > 0);
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    // Keep the original runtime-loop witness independent of proof polls.
    controls.store(0, Ordering::SeqCst);
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

#[test]
fn closed_predicate_quantified_intervals_do_not_widen_the_row_certificate() {
    let (ev, _, _, pred) =
        input("Init == x=0\nNext == UNCHANGED x\nP == \\A i \\in 1..3 : x[i] >= 0");
    let scope = ev.scope(pred).expect("actual scope");
    assert!(
        ev.closed_predicate_stability_budgeted(&scope, ev.ir().def(pred).body, None)
            .expect("inspection")
    );
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &[(ev.ir().def(pred).body, scope)], None)
            .expect("original inspection")
            .is_none(),
        "old canonical producer contract unchanged"
    );
}

#[test]
fn closed_predicate_inspection_retains_unknown_history_effect_and_scope_refusals() {
    for predicate in [
        "LET s == {x,1} IN x \\in s",
        "[i \\in {x,1} |-> i] = x",
        "(CHOOSE i \\in {x,1} : TRUE) = x",
        "[i \\in 1..3 |-> i] = x",
        "(CHOOSE i \\in 1..3 : TRUE) = x",
        "[x EXCEPT ![1] = @+1] = x",
        "PrintT(x=x)",
        "ENABLED (x'=x)",
        "(\"a\" \\o \"b\") = x",
    ] {
        let (ev, _, _, pred) = input(&format!(
            "Init == x=0\nNext == UNCHANGED x\nP == {predicate}"
        ));
        let scope = ev.scope(pred).expect("actual scope");
        assert!(
            !ev.closed_predicate_stability_budgeted(&scope, ev.ir().def(pred).body, None)
                .expect("inspection"),
            "{predicate}"
        );
        assert!(ev.take_output().is_empty(), "inspection does not evaluate");
        let foreign =
            Evaluator::new(Model::new(ev.ir().clone()), Options::tlc()).expect("foreign evaluator");
        assert!(
            !foreign
                .closed_predicate_stability_budgeted(&scope, ev.ir().def(pred).body, None)
                .expect("foreign scope refuses")
        );
    }
}

#[test]
fn closed_predicate_scope_values_require_closed_scalar_keyed_function_storage() {
    let (ev, _, _, pred) = input("Init == x=0\nNext == UNCHANGED x\nP(a) == a=a");
    let body = ev.ir().def(pred).body;
    let function = Value::fcn(
        vec![
            (Value::Int(0), Value::tuple(vec![Value::Int(1)])),
            (Value::Int(2), Value::Bool(true)),
        ],
        ev.options().ctx,
    )
    .expect("closed general function");
    let mut scope = ev.scope(pred).expect("scope");
    scope.env.bind(0, Sv::Val(function));
    assert!(
        ev.closed_predicate_stability_budgeted(&scope, body, None)
            .expect("closed inspection")
    );
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &[(body, scope.clone())], None)
            .expect("old inspection")
            .is_none()
    );
    for value in [
        Value::fcn(
            vec![(Value::tuple(vec![Value::Int(0)]), Value::Int(1))],
            ev.options().ctx,
        )
        .expect("aggregate key"),
        Value::fcn(
            vec![(
                Value::Int(0),
                Value::set_raw(vec![Value::Int(1)], ev.options().ctx),
            )],
            ev.options().ctx,
        )
        .expect("raw nested value"),
    ] {
        scope.env.bind(0, Sv::Val(value));
        assert!(
            !ev.closed_predicate_stability_budgeted(&scope, body, None)
                .expect("unproved storage")
        );
    }
}

#[test]
fn closed_predicate_checks_nested_function_context_and_stored_domain_canonicality() {
    let (ev, _, _, pred) = input("Init == x=0\nNext == UNCHANGED x\nP(a) == a=a");
    let body = ev.ir().def(pred).body;
    let mut scope = ev.scope(pred).expect("scope");
    let canonical = Value::fcn(
        vec![
            (Value::Int(0), Value::Int(1)),
            (Value::Int(2), Value::Int(2)),
        ],
        ev.options().ctx,
    )
    .expect("canonical function");
    for value in [
        canonical.clone(),
        Value::tuple(vec![canonical.clone()]),
        Value::tuple(vec![Value::tuple(vec![canonical])]),
    ] {
        scope.env.bind(0, Sv::Val(value));
        assert!(
            ev.closed_predicate_stability_budgeted(&scope, body, None)
                .expect("canonical inspection")
        );
    }
    let foreign = Value::fcn(
        vec![
            (Value::Int(0), Value::Int(1)),
            (Value::Int(2), Value::Int(2)),
        ],
        Ctx::dive(),
    )
    .expect("foreign ordering");
    for value in [
        foreign.clone(),
        Value::tuple(vec![foreign.clone()]),
        Value::tuple(vec![Value::tuple(vec![foreign])]),
    ] {
        scope.env.bind(0, Sv::Val(value));
        assert!(
            !ev.closed_predicate_stability_budgeted(&scope, body, None)
                .expect("foreign context declines")
        );
    }
    // Lossless wire values retain the supplied domain order and multiplicity.
    for bytes in [
        [0x2b, 2, 2, 4, 2, 20, 2, 0, 2, 0], // 2->10, 0->0: unsorted
        [0x2b, 2, 2, 4, 2, 20, 2, 4, 2, 0], // 2->10, 2->0: duplicate
    ] {
        let function = dive_value::codec::decode(&mut bytes.as_slice(), &mut |token| {
            ev.core.names.resolve(token)
        })
        .expect("lossless scalar function");
        let before = function
            .as_fcn()
            .expect("decoded function")
            .iter()
            .map(|(key, _)| key)
            .collect::<Vec<_>>();
        for value in [function.clone(), Value::tuple(vec![function.clone()])] {
            scope.env.bind(0, Sv::Val(value));
            assert!(
                !ev.closed_predicate_stability_budgeted(&scope, body, None)
                    .expect("uncanonical domain declines")
            );
        }
        let after = function.as_fcn().expect("decoded function");
        assert_eq!(
            before,
            after.iter().map(|(key, _)| key).collect::<Vec<_>>(),
            "inspection never repairs the domain"
        );
    }
}
