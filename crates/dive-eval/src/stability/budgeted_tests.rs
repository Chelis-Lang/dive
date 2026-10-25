//! Actual evaluator/storage controls for the budget-backed proof route.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "real source/storage test setup"
)]

use super::*;
use crate::{ErrorClass, Host, HostControl, HostError, Interrupted, Model, Options};
use dive_value::{Ctx, DeferredKind, DeferredSet, ValueError};
use std::sync::atomic::{AtomicUsize, Ordering};

fn input(body: &str) -> (Evaluator, dive_ir::DefId) {
    let source = format!(
        "---- MODULE Budgeted ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLE x\n{body}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("Budgeted.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("actual source IR"));
    let def = ir.root_def("P").expect("P");
    let ev = Evaluator::new(Model::new(ir), Options::tlc()).expect("actual evaluator");
    (ev, def)
}

fn root(ev: &Evaluator, def: dive_ir::DefId) -> (ExprId, Scope) {
    (ev.ir().def(def).body, ev.scope(def).expect("actual scope"))
}

fn nested() -> Value {
    (0..300).fold(Value::Int(1), |value, _| Value::tuple(vec![value]))
}

#[test]
fn deep_real_storage_and_low_old_work_are_separate_from_run_budgets() {
    let (ev, def) = input("P(a) == a=a");
    let (expr, mut scope) = root(&ev, def);
    scope.env.bind(0, Sv::Val(nested()));
    let roots = [(expr, scope)];
    assert!(ev.canonical_producer_stability(&[], &roots, 0).is_none());
    assert!(
        ev.canonical_producer_stability(&[], &roots, 100_000)
            .is_none(),
        "old depth contract retained"
    );
    let proof = ev
        .canonical_producer_stability_budgeted(&[], &roots, None)
        .expect("no budget interruption")
        .expect("canonical depth is not a count ceiling");
    assert!(proof.belongs_to(&ev));
    let foreign =
        Evaluator::new(Model::new(ev.ir().clone()), Options::tlc()).expect("foreign Core");
    assert!(!proof.belongs_to(&foreign));
    assert!(
        foreign
            .canonical_producer_stability_budgeted(&[], &roots, None)
            .expect("foreign scope is a refusal")
            .is_none()
    );
}

#[test]
fn deep_source_transfers_and_real_callback_closures_use_the_iterative_route() {
    let expression = format!("{}x{}", "<< ".repeat(180), " >>".repeat(180));
    let (ev, def) = input(&format!("P == {expression} = {expression}"));
    let roots = [root(&ev, def)];
    assert!(
        ev.canonical_producer_stability(&[], &roots, 100_000)
            .is_none()
    );
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &roots, None)
            .expect("deep planning")
            .is_some()
    );
    let (ev, def) = input("Less(a,b) == a<b\nP == SortSeq(<<2,1>>,Less) = <<1,2>>");
    let roots = [root(&ev, def)];
    assert!(
        ev.canonical_producer_stability(&[], &roots, 100_000)
            .is_some()
    );
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &roots, None)
            .expect("actual callback capture proof")
            .is_some()
    );
    assert_eq!(
        ev.eval_def(def, &St::empty())
            .expect("original native callback still runs"),
        Value::TRUE
    );
}

#[derive(Debug)]
struct StopAfter {
    calls: AtomicUsize,
    after: usize,
    token: u64,
}
impl HostControl for StopAfter {
    fn check(&self) -> Result<(), Interrupted> {
        if self.calls.fetch_add(1, Ordering::SeqCst) >= self.after {
            Err(Interrupted::new(self.token))
        } else {
            Ok(())
        }
    }
}

#[test]
fn explicit_and_installed_controls_keep_the_exact_midwalk_interruption() {
    let (ev, def) = input("P(a) == a=a");
    let (expr, mut scope) = root(&ev, def);
    scope.env.bind(0, Sv::Val(nested()));
    let roots = [(expr, scope)];
    let control = Arc::new(StopAfter {
        calls: AtomicUsize::new(0),
        after: 10,
        token: 713,
    });
    let error = ev
        .canonical_producer_stability_budgeted(&[], &roots, Some(control.as_ref()))
        .expect_err("actual traversal must reach explicit control");
    assert_eq!(error.class(), ErrorClass::Interrupted);
    assert_eq!(error.interruption(), Some(Interrupted::new(713)));
    assert_eq!(control.calls.load(Ordering::SeqCst), 11);
    let installed = Arc::new(StopAfter {
        calls: AtomicUsize::new(0),
        after: 10,
        token: 719,
    });
    let guard = ev
        .host_control(Some(installed.clone()))
        .expect("install actual control");
    let error = ev
        .canonical_producer_stability_budgeted(&[], &roots, None)
        .expect_err("None must poll the installed run control");
    assert_eq!(error.interruption(), Some(Interrupted::new(719)));
    assert_eq!(installed.calls.load(Ordering::SeqCst), 11);
    drop(guard);
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &roots, None)
            .expect("control restored")
            .is_some()
    );
}

#[derive(Debug)]
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
    fn member(&self, _value: &Value, _ctx: Ctx) -> Option<Result<bool, ValueError>> {
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

#[test]
fn unknown_provider_inspection_never_calls_even_a_warmed_provider() {
    let (ev, def) = input("P(a) == a=a");
    let calls = Arc::new(AtomicUsize::new(0));
    let value = Value::deferred(Arc::new(Probe(calls.clone())), Ctx::tlc());
    let (expr, mut scope) = root(&ev, def);
    scope.env.bind(0, Sv::Val(value.clone()));
    let roots = [(expr, scope)];
    let before = calls.load(Ordering::SeqCst);
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &roots, None)
            .expect("unknown storage")
            .is_none()
    );
    assert_eq!(calls.load(Ordering::SeqCst), before);
    let Value::Set(set) = value else {
        panic!("actual deferred set");
    };
    assert_eq!(
        set.elements(Ctx::tlc())
            .expect("positive real provider warmup")
            .len(),
        1
    );
    let warmed = calls.load(Ordering::SeqCst);
    assert!(warmed > before);
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &roots, None)
            .expect("cached provider remains unknown")
            .is_none()
    );
    assert_eq!(calls.load(Ordering::SeqCst), warmed);
}

#[derive(Debug)]
struct Permission(Arc<AtomicUsize>);
impl Host for Permission {
    fn check_native_access(&self, _op: StdOp) -> Result<(), HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn general_nonstring_transfers_do_not_evaluate_native_arguments_or_hide_effects() {
    for expression in [
        "<<1>> \\o <<2>>",
        "1 \\o 2",
        "(IF x=0 THEN <<1>> ELSE <<2>>) \\o <<3>>",
    ] {
        let (ev, def) = input(&format!("P == ({expression}) = x"));
        let calls = Arc::new(AtomicUsize::new(0));
        ev.set_host(Arc::new(Permission(calls.clone())));
        assert!(
            ev.canonical_producer_stability_budgeted(&[], &[root(&ev, def)], None)
                .expect("general canonical nonstring transfer")
                .is_some()
        );
        assert_eq!(calls.load(Ordering::SeqCst), 0);
        let _actual = ev.eval_def(def, &St::state(Arc::from([Some(Value::Int(0))])));
        assert!(
            calls.load(Ordering::SeqCst) > 0,
            "ordinary evaluation genuinely reaches the native"
        );
    }
    for expression in ["\"a\" \\o \"b\"", "<<PrintT(TRUE)>> \\o <<1>>"] {
        let (ev, def) = input(&format!("P == ({expression}) = x"));
        assert!(
            ev.canonical_producer_stability_budgeted(&[], &[root(&ev, def)], None)
                .expect("unknown names/effects refuse")
                .is_none()
        );
        assert_eq!(ev.take_output(), [] as [String; 0]);
    }
}

#[test]
fn checked_traversal_allocation_is_a_resource_error_not_an_unknown_certificate() {
    let error = budgeted::reserve(&mut Vec::<u64>::new(), usize::MAX)
        .expect_err("real capacity overflow without a large allocation");
    assert_eq!(error.class(), ErrorClass::ResourceLimit);
    assert!(error.interruption().is_none());
}

#[test]
fn strict_substitution_refuses_proof_and_ordinary_binding_still_produces_a_value() {
    let (ev, def) = input(
        "---- MODULE Inner ----\nEXTENDS Sequences\nCONSTANT K\nRead == Len(K)=1\n====\nI == INSTANCE Inner WITH K <- <<1>>\nP == I!Read",
    );
    assert!(ev.ir().nodes().iter().any(|node| matches!(&node.expr,
        Expr::Subst { bindings, .. } if bindings.iter().any(|binding| matches!(binding.value,
            Arg::Expr { pass: dive_ir::Pass::Strict, .. })))));
    let calls = Arc::new(AtomicUsize::new(0));
    ev.set_host(Arc::new(Permission(calls.clone())));
    let roots = [root(&ev, def)];
    assert!(
        ev.canonical_producer_stability(&[], &roots, 100_000)
            .is_none()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    let (at, declaration) = ev
        .ir()
        .nodes()
        .iter()
        .enumerate()
        .find_map(|(at, node)| {
            let Expr::Subst { bindings, .. } = &node.expr else {
                return None;
            };
            bindings.iter().find_map(|binding| {
                matches!(
                    binding.value,
                    Arg::Expr {
                        pass: dive_ir::Pass::Strict,
                        ..
                    }
                )
                .then_some((at, binding.decl))
            })
        })
        .expect("genuine lowered strict substitution");
    let expression = ExprId(u32::try_from(at).expect("actual expression index"));
    let owner = ev
        .ir()
        .defs()
        .iter()
        .enumerate()
        .find_map(|(at, definition)| (definition.body == expression).then_some(at))
        .expect("source definition owns its substitution");
    let source = ev
        .scope(dive_ir::DefId(
            u32::try_from(owner).expect("actual definition index"),
        ))
        .expect("actual source scope");
    let bound = ev
        .scope_subst(&source, expression)
        .expect("ordinary strict binding helper");
    let SlotView::Value(value) = ev.scope_subst_slot(&bound, declaration) else {
        panic!("strict tuple literal must already be stored, not a lazy binding");
    };
    assert_eq!(value, Value::tuple(vec![Value::Int(1)]));
    assert_eq!(
        calls.load(Ordering::SeqCst),
        0,
        "literal binding needs no native callback"
    );
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &roots, None)
            .expect("strict metadata is unknown, not evaluated")
            .is_none()
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert_eq!(
        ev.eval_def(def, &St::empty())
            .expect("ordinary binding remains live"),
        Value::TRUE
    );
    assert!(
        calls.load(Ordering::SeqCst) > 0,
        "ordinary substituted body genuinely reaches Len after binding"
    );
}

#[test]
fn successful_warm_thunks_do_not_replace_the_original_capture_proof() {
    let (ev, def) = input("F(a) == a /\\ a\nP == F(PrintT(TRUE))");
    let target = ev.ir().root_def("F").expect("actual F");
    let Expr::Apply { args, .. } = ev.ir().expr(ev.ir().def(def).body) else {
        panic!("actual lowered call");
    };
    let scope = ev
        .scope_apply(&ev.scope(def).expect("source scope"), target, args)
        .expect("real captured argument");
    let body = ev.ir().def(target).body;
    assert_eq!(
        ev.eval_scoped(&scope, body, &St::empty())
            .expect("positive actual warmup"),
        Value::TRUE
    );
    assert_eq!(ev.take_output(), ["TRUE"]);
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &[(body, scope)], None)
            .expect("effectful original capture stays unknown")
            .is_none()
    );
    assert_eq!(ev.take_output(), [] as [String; 0]);
}
