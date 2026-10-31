//! Actual evaluator/storage controls for the budget-backed proof route.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "real source/storage test setup"
)]

use super::budgeted;
use crate::env::Sv;
use crate::{ErrorClass, Host, HostControl, HostError, Interrupted, Model, Options};
use crate::{Evaluator, Scope, SlotView, St};
use dive_ir::{Arg, Expr, ExprId, StdOp};
use dive_value::Value;
use dive_value::{Ctx, DeferredKind, DeferredSet, ValueError};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

fn lower_input(body: &str) -> (Arc<dive_ir::Ir>, dive_ir::DefId) {
    let source = format!(
        "---- MODULE Budgeted ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLE x\n{body}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("Budgeted.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("actual source IR"));
    let def = ir.root_def("P").expect("P");
    (ir, def)
}

fn input(body: &str) -> (Evaluator, dive_ir::DefId) {
    let (ir, def) = lower_input(body);
    (
        Evaluator::new(Model::new(ir), Options::tlc()).expect("actual evaluator"),
        def,
    )
}

fn root(ev: &Evaluator, def: dive_ir::DefId) -> (ExprId, Scope) {
    (ev.ir().def(def).body, ev.scope(def).expect("actual scope"))
}

fn nested() -> Value {
    (0..300).fold(Value::Int(1), |value, _| Value::tuple(vec![value]))
}

#[test]
fn deep_real_storage_is_inspected_under_the_run_budget() {
    let (ev, def) = input("P(a) == a=a");
    let (expr, mut scope) = root(&ev, def);
    scope.env.bind(0, Sv::Val(nested()));
    let roots = [(expr, scope)];
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
    let body = format!("P == {expression} = {expression}");
    // Analysis/lowering is fixture construction, not the proof under test.
    // Return only immutable IR; evaluator construction and the proof call
    // below stay on this original test thread's ordinary stack.
    let (ir, def) = std::thread::Builder::new()
        .name("canonical-deep-source-fixture".into())
        .stack_size(16 * 1024 * 1024)
        .spawn(move || lower_input(&body))
        .expect("bounded fixture worker")
        .join()
        .expect("fixture analysis/lowering completed");
    let ev = Evaluator::new(Model::new(ir), Options::tlc()).expect("actual evaluator");
    let roots = [root(&ev, def)];
    assert!(
        ev.canonical_producer_stability_budgeted(&[], &roots, None)
            .expect("deep planning")
            .is_some()
    );
    let (ev, def) = input("Less(a,b) == a<b\nP == SortSeq(<<2,1>>,Less) = <<1,2>>");
    let roots = [root(&ev, def)];
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
        ev.canonical_producer_stability_budgeted(&[], &roots, None)
            .expect("strict metadata is refused before ordinary binding")
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

#[test]
fn guarded_interval_construction_is_source_inspection_not_new_producer_authority() {
    for body in [
        "P == x'=[i \\in 0..2 |-> i+1]",
        "P == x'=[i \\in 1..Len(x) |-> x[i]]",
        "P == x'=[i \\in 2..1 |-> 1 \\div 0]",
        "Node == 0..2\nP == x'=[i \\in Node |-> i]",
        "P == x'=(CHOOSE i \\in 1..Len(x): x[i]=0)",
        "P == x'=(CHOOSE i \\in 1..3: FALSE)",
        "P == \\E i \\in (0..2) \\ {0}: x'=[x EXCEPT ![i]=@+1]",
        "P == \\E i \\in 0..2: \\E j \\in (0..2) \\ {i}: x'=[x EXCEPT ![j]=@-1]",
        "P == x'=[[i \\in 0..2 |-> i] EXCEPT ![0]=@+1]",
        "P == x'=[x EXCEPT ![0]=LET old == @ IN old+1]",
    ] {
        let (ev, def) = input(body);
        let (expr, scope) = root(&ev, def);
        assert!(
            ev.guarded_action_stability_budgeted(&scope, expr, None)
                .expect("source inspection"),
            "{body}"
        );
        assert!(
            !ev.closed_predicate_stability_budgeted(&scope, expr, None)
                .expect("old predicate boundary"),
            "{body}"
        );
        assert!(
            ev.canonical_producer_stability_budgeted(&[], &[(expr, scope.clone())], None)
                .expect("old producer boundary")
                .is_none(),
            "{body}"
        );
        let foreign =
            Evaluator::new(Model::new(ev.ir().clone()), Options::tlc()).expect("foreign owner");
        assert!(
            !foreign
                .guarded_action_stability_budgeted(&scope, expr, None)
                .expect("foreign refuses")
        );
        assert_eq!(ev.take_output(), [] as [String; 0]);
    }
}

#[test]
fn scalar_choice_inspection_consumes_only_its_own_proved_integer_domain() {
    for body in [
        "P == x'=(CHOOSE i \\in {1,2}: i>5)",
        "P == x'=(CHOOSE i \\in {2,1,2}: i>1)",
        "P == x'=(CHOOSE i \\in {}: TRUE)",
        "P == x'=(CHOOSE i \\in {1,2}: IF i=1 THEN FALSE ELSE (1 \\div 0)=0)",
        "P == LET bound == 2 IN x'=(CHOOSE i \\in {1,bound}: i>0)",
    ] {
        let (ev, def) = input(body);
        let roots = [root(&ev, def)];
        assert!(
            ev.canonical_producer_stability_budgeted(&roots, &[], None)
                .expect("budgeted inspection")
                .is_some(),
            "{body}"
        );
        assert_eq!(ev.take_output(), [] as [String; 0]);
    }
    for body in [
        "P == x'=(CHOOSE i \\in {TRUE,1}: TRUE)",
        "P == x'=(CHOOSE i \\in {x,1}: TRUE)",
        "Domain == {1,2}\\nP == x'=(CHOOSE i \\in Domain: TRUE)",
        "P == x'=(CHOOSE i \\in {1,2}: PrintT(TRUE))",
        "P == x'={1,2}",
        "P == x'=(CHOOSE i \\in {<<1>>,<<2>>}: TRUE)",
    ] {
        let (ev, def) = input(&body.replace("\\n", "\n"));
        assert!(
            ev.canonical_producer_stability_budgeted(&[root(&ev, def)], &[], None)
                .expect("conservative inspection")
                .is_none(),
            "{body}"
        );
        assert_eq!(ev.take_output(), [] as [String; 0]);
    }
}

#[test]
fn guarded_constructors_do_not_admit_unknown_domains_nested_updates_or_capture_history() {
    for body in [
        "P == x'=[i \\in {0,1} |-> i]",
        "P == x'=[i \\in x |-> i]",
        "P == x'=[i \\in 0..2,j \\in 0..2 |-> i+j]",
        "P == x'=(CHOOSE i \\in {0,1}:TRUE)",
        "P == x'=(CHOOSE i \\in x:TRUE)",
        "P == \\E i \\in (0..2) \\ {x}: x'=i",
        "P == \\E i \\in (0..2) \\ {TRUE}: x'=i",
        "P == x'=[x EXCEPT ![0]=0, ![1]=1]",
        "P == x'=[x EXCEPT ![0][1]=0]",
        "P == x'=[[x EXCEPT ![0]=1] EXCEPT ![1]=2]",
        "P == x'=[x EXCEPT ![0]=[x EXCEPT ![1]=@+1]]",
        "P == x'=[i \\in 0..2 |-> PrintT(TRUE)]",
        "P == x'=(CHOOSE i \\in 0..2:PrintT(TRUE))",
        "P == x'=[i \\in 0..2 |-> \"a\" \\o \"b\"]",
        "P == x'=[0..2 -> BOOLEAN]",
        "P == x'=[a:0..2]",
    ] {
        let (ev, def) = input(body);
        let (expr, scope) = root(&ev, def);
        assert!(
            !ev.guarded_action_stability_budgeted(&scope, expr, None)
                .expect("unproved source refuses"),
            "{body}"
        );
        assert_eq!(ev.take_output(), [] as [String; 0]);
    }
    let (ev, def) = input("P(unused) == x'=[x EXCEPT ![0]=@+1]");
    let (expr, mut scope) = root(&ev, def);
    scope.env.bind(
        0,
        Sv::Val(Value::set_raw(vec![Value::Int(1)], ev.options().ctx)),
    );
    assert!(
        !ev.guarded_action_stability_budgeted(&scope, expr, None)
            .expect("unused raw capture refuses")
    );
    let (ev, def) = input("P == x'=[x EXCEPT ![0]=@+1]");
    let (expr, mut scope) = root(&ev, def);
    scope.env.dynamic.at = Some(Value::Int(7));
    assert!(
        !ev.guarded_action_stability_budgeted(&scope, expr, None)
            .expect("real dynamic at is never synthetic lexical authority")
    );
}

#[test]
fn guarded_construction_inspection_preserves_actual_controls_and_never_calls_host() {
    let (ev, def) = input("P == x'=[i \\in 1..Len(x) |-> i+1]");
    let (expr, scope) = root(&ev, def);
    let calls = Arc::new(AtomicUsize::new(0));
    ev.set_host(Arc::new(Permission(calls.clone())));
    let control = StopAfter {
        calls: AtomicUsize::new(0),
        after: 12,
        token: 911,
    };
    let error = ev
        .guarded_action_stability_budgeted(&scope, expr, Some(&control))
        .expect_err("actual bounded inspection interruption");
    assert_eq!(error.interruption(), Some(Interrupted::new(911)));
    assert_eq!(calls.load(Ordering::SeqCst), 0);
    assert!(
        ev.guarded_action_stability_budgeted(&scope, expr, None)
            .expect("fresh walk resets")
    );
    assert_eq!(calls.load(Ordering::SeqCst), 0);
}

#[test]
fn guarded_scoped_projection_preserves_lazy_instance_and_unused_capture_boundaries() {
    for tlc in [false, true] {
        let (ir, def) = lower_input(
            "---- MODULE Inner ----\nVARIABLE projected\nvars == projected\nNext == projected'=[projected EXCEPT ![0]=@+1]\n====\n\
             I == INSTANCE Inner WITH projected <- [i \\in 0..Len(x) |-> i]\nP == I!Next",
        );
        let ev = Evaluator::new(
            Model::new(ir),
            if tlc { Options::tlc() } else { Options::dive() },
        )
        .expect("actual profile");
        let (expr, scope) = root(&ev, def);
        assert!(
            ev.guarded_action_stability_budgeted(&scope, expr, None)
                .expect("actual lazy projection")
        );
        assert!(
            !ev.closed_predicate_stability_budgeted(&scope, expr, None)
                .expect("old predicate stays refused")
        );
        assert!(
            ev.canonical_producer_stability_budgeted(&[], &[(expr, scope)], None)
                .expect("old producer stays refused")
                .is_none()
        );
    }
    let (ev, def) = input("F(unused) == x'=[x EXCEPT ![0]=@+1]\nP == F(PrintT(TRUE))");
    let (expr, scope) = root(&ev, def);
    assert!(
        !ev.guarded_action_stability_budgeted(&scope, expr, None)
            .expect("unused original callback is still inspected")
    );
    assert_eq!(ev.take_output(), [] as [String; 0]);
}

#[test]
fn constructor_inspection_does_not_force_keep_lazy_or_change_reference_choose_order() {
    for tlc in [false, true] {
        for domain in ["0..2", "1..3", "2..1"] {
            let (ir, def) = lower_input(&format!("P == [i \\in {domain} |-> i+1]"));
            let ev = Evaluator::new(
                Model::new(ir),
                if tlc { Options::tlc() } else { Options::dive() },
            )
            .expect("profile");
            let (expr, scope) = root(&ev, def);
            assert!(
                ev.guarded_action_stability_budgeted(&scope, expr, None)
                    .expect("source closure")
            );
            let value = ev
                .core
                .eval(&mut scope.env.clone(), &St::empty(), expr)
                .expect("ordinary constructor");
            let crate::env::Ev::V(Value::Fcn(function)) = value else {
                panic!("reducible interval must materialize without KEEP_LAZY");
            };
            assert!(
                !function.is_tuple_literal(),
                "interval layout does not manufacture tuple literal class"
            );
            assert!(!function.is_record_literal());
            assert!(matches!(
                ev.core
                    .eval(
                        &mut scope.env.clone(),
                        &St::empty().with_control(crate::env::control::KEEP_LAZY),
                        expr
                    )
                    .expect("lazy constructor"),
                crate::env::Ev::F(_)
            ));
        }
        for (body, expected) in [
            ("CHOOSE i \\in 1..3: i>1", Some(Value::Int(2))),
            ("CHOOSE i \\in 1..3: TRUE", Some(Value::Int(1))),
            ("CHOOSE i \\in 2..1: TRUE", None),
            ("CHOOSE i \\in 1..3: FALSE", None),
        ] {
            let (ir, def) = lower_input(&format!("P == {body}"));
            let ev = Evaluator::new(
                Model::new(ir),
                if tlc { Options::tlc() } else { Options::dive() },
            )
            .expect("profile");
            let (expr, scope) = root(&ev, def);
            assert!(
                ev.guarded_action_stability_budgeted(&scope, expr, None)
                    .expect("inspection never chooses a witness")
            );
            match expected {
                Some(value) => assert_eq!(
                    ev.eval_def(def, &St::empty()).expect("first witness"),
                    value
                ),
                None => assert!(
                    ev.eval_def(def, &St::empty()).is_err(),
                    "no-witness error is not hidden by source certificate"
                ),
            }
        }
    }
}
