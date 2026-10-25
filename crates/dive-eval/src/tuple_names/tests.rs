//! Real source, storage and interruption boundaries of the optional fact.
#![allow(clippy::expect_used, clippy::panic, reason = "real evaluator fixtures")]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use dive_ir::{DefId, ExprId, StdOp};
use dive_value::{Ctx, DeferredKind, DeferredSet, Value, ValueError};

use crate::env::Sv;
use crate::{Evaluator, Host, HostControl, HostError, Interrupted, Model, Options, Scope, St};

fn input(body: &str) -> (Evaluator, DefId) {
    let source = format!(
        "---- MODULE Names ----\nEXTENDS Naturals, Sequences, TLC\nVARIABLE x\n{body}\n====\n"
    );
    let analysis = dive_sema::analyze_source("Names.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{analysis:?}");
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let pred = ir.root_def("P").expect("P");
    (
        Evaluator::new(Model::new(ir), Options::tlc()).expect("evaluator"),
        pred,
    )
}

fn root(ev: &Evaluator, def: DefId) -> (ExprId, Scope) {
    (ev.ir().def(def).body, ev.scope(def).expect("actual scope"))
}

fn proves(ev: &Evaluator, def: DefId) -> bool {
    ev.tuple_concat_sources(&[root(ev, def)], &[], None)
        .expect("metadata inspection")
        .is_some_and(|fact| fact.belongs_to(ev))
}

#[test]
fn literal_append_and_branch_results_are_grounded_without_parameter_types() {
    for body in [
        "P == <<x>> \\o x",
        "Alias(a) == Append(a,x)\nP == Alias(x) \\o x",
        "F(a) == IF x=0 THEN <<a>> ELSE Append(a,x)\nP == F(x) \\o x",
        "F(a) == CASE x=0 -> <<a>> [] OTHER -> Append(a,x)\nP == F(x) \\o x",
        "P == LET A == <<x>> IN A \\o x",
    ] {
        let (ev, pred) = input(body);
        assert!(proves(&ev, pred), "{body}");
    }
    for body in [
        "F(a) == IF x=0 THEN <<a>> ELSE a\nP == F(x) \\o x",
        "P == x \\o x",
        "P == (<<x>> \\o x) = (x \\o x)",
    ] {
        let (ev, pred) = input(body);
        assert!(!proves(&ev, pred), "{body}");
    }
}

#[test]
fn recursive_results_need_a_constructor_ground_not_a_recursive_assumption() {
    let (ev, pred) =
        input("RECURSIVE F(_)\nF(a) == IF a=0 THEN <<>> ELSE Append(F(a-1),a)\nP == F(x) \\o x");
    assert!(proves(&ev, pred));
    assert_eq!(
        ev.eval_def(pred, &St::state(Arc::from([Some(Value::Int(0))])))
            .expect_err("tuple/integer is still a genuine runtime error")
            .class(),
        crate::ErrorClass::Eval
    );
    let (ev, pred) = input("RECURSIVE F(_)\nF(a) == IF a=0 THEN <<>> ELSE F(a-1)\nP == F(x) \\o x");
    assert!(
        !proves(&ev, pred),
        "a recursive edge alone never becomes a tuple fact"
    );
}

#[derive(Debug)]
struct Permissions(Arc<AtomicUsize>);
impl Host for Permissions {
    fn check_native_access(&self, _op: StdOp) -> Result<(), HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
}

#[test]
fn child_calls_and_effects_are_inspected_without_executing_them() {
    let (ev, pred) = input("P == <<PrintT(x)>> \\o <<x>>");
    let permissions = Arc::new(AtomicUsize::new(0));
    ev.set_host(Arc::new(Permissions(permissions.clone())));
    assert!(proves(&ev, pred));
    assert_eq!(permissions.load(Ordering::SeqCst), 0);
    assert_eq!(ev.take_output(), Vec::<String>::new());
    let _ = ev
        .eval_def(pred, &St::state(Arc::from([Some(Value::Int(0))])))
        .expect("ordinary execution remains effectful");
    assert!(permissions.load(Ordering::SeqCst) > 0);
    assert_eq!(ev.take_output(), ["0"]);
    let (ev, pred) = input("P == <<ToString(x)>> \\o <<x>>");
    assert!(
        !proves(&ev, pred),
        "a tuple container cannot hide its child's name creator"
    );
}

#[test]
fn higher_order_calls_and_bare_native_roots_remain_unknown() {
    let (ev, pred) = input("F(Op(_),a) == Op(a)\nP == <<x>> \\o F(Len,x)");
    assert!(!proves(&ev, pred));
    let (ev, pred) = input("P == <<x>> \\o <<x>>");
    let native = ev
        .ir()
        .defs()
        .iter()
        .enumerate()
        .find_map(|(index, def)| {
            (def.std == Some(StdOp::SequencesConcat))
                .then(|| DefId(u32::try_from(index).expect("definition index")))
        })
        .expect("actual standard concat definition");
    assert!(
        ev.tuple_concat_sources(&[root(&ev, pred)], &[native], None)
            .expect("native root")
            .is_none()
    );
}

#[test]
fn actual_native_redirects_cannot_borrow_the_original_constructor_fact() {
    let (original, pred) = input("Replacement(a,b) == a\nP == Append(x,x) \\o x");
    assert!(proves(&original, pred));
    let replacement = original.ir().root_def("Replacement").expect("replacement");
    let append = original
        .ir()
        .defs()
        .iter()
        .enumerate()
        .find_map(|(index, def)| {
            (def.std == Some(StdOp::SequencesAppend))
                .then(|| DefId(u32::try_from(index).expect("definition index")))
        })
        .expect("actual Append definition");
    let mut model = Model::new(original.ir().clone());
    model.redirects.insert(append, replacement);
    let redirected = Evaluator::new(model, Options::tlc()).expect("actual configured redirect");
    assert!(!proves(&redirected, pred));
}

#[test]
fn shared_value_dags_and_long_transparent_aliases_have_no_count_admission_limit() {
    let (ev, pred) = input("P(a) == <<x>> \\o <<x>>");
    let mut value = Value::Int(0);
    for _ in 0..300 {
        value = Value::tuple(vec![value.clone(), value]);
    }
    let mut source = root(&ev, pred);
    source.1.env.bind(0, Sv::Val(value));
    assert!(
        ev.tuple_concat_sources(&[source], &[], None)
            .expect("shared DAG inspection")
            .is_some()
    );
    let mut body = String::from("A0 == Append(x,x)\n");
    for index in 1..300 {
        use std::fmt::Write;
        writeln!(&mut body, "A{index} == A{}", index - 1).expect("source buffer");
    }
    body.push_str("P == A299 \\o x");
    let (ev, pred) = input(&body);
    assert!(proves(&ev, pred));
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
fn cold_and_warm_opaque_storage_is_not_queried_even_when_unused() {
    let (ev, pred) = input("P(a) == <<x>> \\o <<x>>");
    let count = Arc::new(AtomicUsize::new(0));
    let value = Value::deferred(Arc::new(Probe(count.clone())), Ctx::tlc());
    let mut source = root(&ev, pred);
    source.1.env.bind(0, Sv::Val(value.clone()));
    let before = count.load(Ordering::SeqCst);
    assert!(
        ev.tuple_concat_sources(&[source.clone()], &[], None)
            .expect("opaque refusal")
            .is_none()
    );
    assert_eq!(count.load(Ordering::SeqCst), before);
    let Value::Set(set) = value else {
        panic!("set")
    };
    let _ = set.elements(Ctx::tlc()).expect("real provider warmup");
    let warm = count.load(Ordering::SeqCst);
    assert!(warm > before);
    assert!(
        ev.tuple_concat_sources(&[source], &[], None)
            .expect("warm refusal")
            .is_none()
    );
    assert_eq!(count.load(Ordering::SeqCst), warm);
}

#[derive(Debug)]
struct Stop;
impl HostControl for Stop {
    fn check(&self) -> Result<(), Interrupted> {
        Err(Interrupted::new(915))
    }
}

#[test]
fn foreign_scopes_and_both_control_sources_preserve_actual_authority() {
    let (ev, pred) = input("P == <<x>> \\o <<x>>");
    let (foreign, other) = input("P == <<x>> \\o <<x>>");
    assert!(
        ev.tuple_concat_sources(&[root(&foreign, other)], &[], None)
            .expect("foreign refusal")
            .is_none()
    );
    let roots = [root(&ev, pred)];
    let explicit = ev
        .tuple_concat_sources(&roots, &[], Some(&Stop))
        .expect_err("explicit control");
    assert_eq!(
        explicit.interruption().expect("typed interruption").token(),
        915
    );
    let _guard = ev
        .host_control(Some(Arc::new(Stop)))
        .expect("installed control");
    let installed = ev
        .tuple_concat_sources(&roots, &[], None)
        .expect_err("installed control");
    assert_eq!(
        installed
            .interruption()
            .expect("typed interruption")
            .token(),
        915
    );
}

#[test]
fn dominated_let_facts_do_not_escape_into_roots_formals_or_rebound_contexts() {
    let (ev, pred) = input("P == LET A == <<x>> IN A \\o x\nQ(a) == a \\o x");
    let entered = root(&ev, pred);
    assert!(
        proves(&ev, pred),
        "the actual entered LET remains a positive"
    );
    let dive_ir::Expr::Let { defs, body } = ev.ir().expr(entered.0) else {
        panic!("real LET root");
    };
    let dive_ir::DefScope::LetValue { slot, .. } = ev.ir().def(defs[0]).scope else {
        panic!("real LET slot");
    };
    let mut forged = entered.1.clone();
    forged
        .env
        .bind(slot.index(), Sv::Val(Value::tuple(vec![Value::Int(0)])));
    let bare = (*body, forged);
    assert!(
        ev.tuple_concat_sources(&[bare.clone()], &[], None)
            .expect("bare slot refusal")
            .is_none()
    );
    for sources in [vec![entered.clone(), bare.clone()], vec![bare, entered]] {
        assert!(
            ev.tuple_concat_sources(&sources, &[], None)
                .expect("mixed root refusal")
                .is_none()
        );
    }
    let count = Arc::new(AtomicUsize::new(0));
    let mut opaque = root(&ev, pred);
    opaque.1.env.bind(
        slot.index(),
        Sv::Val(Value::deferred(Arc::new(Probe(count.clone())), Ctx::tlc())),
    );
    let before = count.load(Ordering::SeqCst);
    assert!(
        ev.tuple_concat_sources(&[opaque], &[], None)
            .expect("opaque capture refusal")
            .is_none()
    );
    assert_eq!(count.load(Ordering::SeqCst), before);
    let other = ev.ir().root_def("Q").expect("formal root");
    assert!(
        ev.tuple_concat_sources(&[root(&ev, pred), root(&ev, other)], &[], None)
            .expect("formal root refusal")
            .is_none()
    );
    for body in [
        "P == LET A == <<x>> F(a) == a \\o x IN F(A)",
        "P == LET A == <<x>> IN \\A b \\in {x}: (A \\o x)=x",
        "P(Op(_)) == LET A == <<x>> IN A \\o Op(x)",
    ] {
        let (ev, pred) = input(body);
        assert!(!proves(&ev, pred), "{body}");
    }
}
