//! Scalar cursor compared with the retained eager route, not an oracle run.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "private finite regression assertions"
)]
use super::*;
use crate::{
    ErrorClass, Host, HostError, Options,
    env::{Sv, take_frame_copies},
};
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

const SOURCE: &str = r#"---- MODULE ScalarCursor ----
EXTENDS Integers, TLC
VARIABLE x
Order == \A i,j \in {1,2}: TRUE
Nested == \A i \in {1,2}: \A j \in 1..(x+i): j <= x+i
Empty == \A i \in {}: \A j \in 1..(1 \div 0): TRUE
LateDomain == \A i \in {}, j \in 1..(1 \div 0): TRUE
ProductLimit == \A i,j \in {1,2}: \A k \in 1..(1 \div 0): TRUE
EarlyScalar == \A i \in {0,1}: \A j \in 1..(1 \div i): TRUE
LateTuple == \A <<a,b>> \in {<<0,1>>, <<2,3,4>>}: \A j \in 1..(1 \div a): TRUE
Query == \A i \in {0,1}: ENABLED (x'=x)
PrimedQuery == \A i \in {0,1}: (ENABLED (x'=x))'
Bad == \A i \in {0,1}: PrintT("never")
====
"#;

#[derive(Debug, Default)]
struct Hooks(AtomicUsize);
impl Host for Hooks {
    fn check_native_access(&self, _: StdOp) -> Result<(), HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(())
    }
    fn print_output(&self, _: &str) -> Result<bool, HostError> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Ok(true)
    }
}
fn evaluator(options: Options) -> (Evaluator, Arc<Hooks>) {
    let a = dive_sema::analyze_source("ScalarCursor.tla", SOURCE, &dive_sema::Options::default());
    assert!(a.accepted(), "{a:?}");
    let ev = Evaluator::new(
        crate::Model::new(Arc::new(dive_ir::lower(&a).expect("IR"))),
        options,
    )
    .expect("evaluator");
    let hooks = Arc::new(Hooks::default());
    ev.set_host(hooks.clone());
    (ev, hooks)
}
fn scope(ev: &Evaluator, name: &str) -> (Scope, ExprId) {
    let d = ev.ir().root_def(name).expect("definition");
    (ev.scope(d).expect("scope"), ev.ir().def(d).body)
}
fn pair(ev: &Evaluator, expr: ExprId, bound: &Scope) -> (dive_value::Value, dive_value::Value) {
    let Expr::Quant { bounds, .. } = ev.ir().expr(expr) else {
        panic!("quantifier")
    };
    let values: Vec<_> = bounds[0]
        .vars
        .iter()
        .map(|slot| match &bound.env.frame.slots[slot.index()] {
            Sv::Val(v) => v.clone(),
            _ => panic!("actual scalar binding"),
        })
        .collect();
    (values[0].clone(), values[1].clone())
}

#[test]
fn binding_order_and_unique_working_frame_avoid_eager_reconstruction() {
    for options in [Options::dive(), Options::tlc()] {
        let (ev, hooks) = evaluator(options);
        let (scope, expr) = scope(&ev, "Order");
        assert_eq!(Arc::strong_count(&scope.env.frame), 1);
        take_frame_copies();
        let eager = ev
            .scope_bindings_bounded(&scope, expr, &St::constant(), 4096)
            .expect("eager");
        assert_eq!(take_frame_copies(), 4);
        let expected: Vec<(dive_value::Value, dive_value::Value)> = vec![
            (1.into(), 1.into()),
            (2.into(), 1.into()),
            (1.into(), 2.into()),
            (2.into(), 2.into()),
        ];
        assert_eq!(
            eager
                .iter()
                .map(|sc| pair(&ev, expr, sc))
                .collect::<Vec<_>>(),
            expected
        );
        drop(eager);
        let mut actual = Vec::new();
        assert_eq!(Arc::strong_count(&scope.env.frame), 1);
        ev.for_each_scalar_quantifier_scope(&scope, expr, &St::constant(), 4096, |sc| {
            assert_eq!(Arc::strong_count(&sc.env.frame), 1);
            actual.push(pair(&ev, expr, sc));
            Ok(())
        })
        .expect("cursor");
        assert_eq!(actual, expected);
        assert_eq!(take_frame_copies(), 1);
        assert_eq!(Arc::strong_count(&scope.env.frame), 1);
        assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        assert_eq!(ev.take_output(), Vec::<String>::new());
    }
}

#[test]
fn actual_retained_snapshot_forces_cow_before_the_next_binding() {
    for options in [Options::dive(), Options::tlc()] {
        let (ev, _) = evaluator(options);
        let (scope, expr) = scope(&ev, "Order");
        let mut snapshot = None;
        let mut order = Vec::new();
        take_frame_copies();
        ev.for_each_scalar_quantifier_scope(&scope, expr, &St::constant(), 4096, |sc| {
            order.push(pair(&ev, expr, sc));
            if snapshot.is_none() {
                assert_eq!(Arc::strong_count(&sc.env.frame), 1);
                snapshot = Some(sc.clone());
                assert_eq!(Arc::strong_count(&sc.env.frame), 2);
            }
            Ok(())
        })
        .expect("cursor with a real retained scope");
        let snapshot = snapshot.expect("first snapshot");
        assert_eq!(pair(&ev, expr, &snapshot), (1.into(), 1.into()));
        assert_eq!(
            order,
            vec![
                (1.into(), 1.into()),
                (2.into(), 1.into()),
                (1.into(), 2.into()),
                (2.into(), 2.into())
            ]
        );
        assert_eq!(take_frame_copies(), 2);
        assert_eq!(ev.take_output(), Vec::<String>::new());
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "independent eager/cursor context and work controls"
)]
fn guard_run(
    ev: &Evaluator,
    name: &str,
    state: i64,
    limit: usize,
    depth: usize,
    analysis: bool,
    query_allowed: bool,
    eager: bool,
) -> (crate::Result<()>, usize, Vec<ExprId>) {
    let (scope, expr) = scope(ev, name);
    let mut guard = Guard {
        evaluator: ev,
        limit,
        visits: 0,
        analysis,
        selectseq: false,
        recursive_state: false,
        configured_depth: 0,
        current_sequence: None,
        functions: Vec::new(),
        query_allowed,
        query_depth: 0,
        eager_quantifiers: eager,
        visit_trace: Some(Vec::new()),
    };
    let result = guard.walk(
        &scope,
        expr,
        &St::state(Arc::from([Some(state.into())])),
        depth,
    );
    (result, guard.visits, guard.visit_trace.expect("trace"))
}

#[test]
fn every_original_guard_visit_error_budget_query_and_analysis_path_matches_eager() {
    for options in [Options::dive(), Options::tlc()] {
        let (ev, hooks) = evaluator(options);
        for name in [
            "Order",
            "Nested",
            "Empty",
            "LateDomain",
            "ProductLimit",
            "EarlyScalar",
            "LateTuple",
            "Query",
            "PrimedQuery",
            "Bad",
        ] {
            for state in [0, 1] {
                for limit in [0, 1, 3, 4096] {
                    for analysis in [false, true] {
                        for query in [false, true] {
                            hooks.0.store(0, Ordering::SeqCst);
                            let old = guard_run(&ev, name, state, limit, 0, analysis, query, true);
                            let old_hooks = hooks.0.swap(0, Ordering::SeqCst);
                            let new = guard_run(&ev, name, state, limit, 0, analysis, query, false);
                            assert_eq!(
                                new, old,
                                "{name}, state{state}, limit{limit}, analysis{analysis}, query{query}"
                            );
                            assert_eq!(hooks.0.swap(0, Ordering::SeqCst), old_hooks);
                            assert_eq!(ev.take_output(), Vec::<String>::new());
                        }
                    }
                }
            }
        }
        let empty = guard_run(&ev, "Empty", 0, 4096, 0, false, true, false);
        assert!(empty.0.is_ok());
        assert_eq!(empty.1, 2);
        let quota = guard_run(&ev, "ProductLimit", 0, 3, 0, false, true, false)
            .0
            .expect_err("Cartesian count before body division");
        assert_eq!(quota, EvalError::resource_limit("quantifier bindings"));
        // A TLC evaluation error in a body's domain skips that binder; the
        // walk continues with the next binding, and evaluation itself still
        // raises the error where TLC does.
        let (skipped, _, trace) = guard_run(&ev, "EarlyScalar", 0, 4096, 0, false, true, false);
        skipped.expect("an erroring body domain is left to evaluation");
        let (_, eager_visits, eager_trace) =
            guard_run(&ev, "EarlyScalar", 0, 4096, 0, false, true, true);
        assert_eq!(trace, eager_trace);
        assert!(eager_visits > 1, "the second binding is still visited");
        let (scope, expr) = scope(&ev, "EarlyScalar");
        let divide = ev
            .eval_scoped(&scope, expr, &St::state(Arc::from([Some(0.into())])))
            .expect_err("evaluation reaches the first body domain");
        assert_eq!(divide.class(), ErrorClass::Eval);
        assert_eq!(divide.ec(), crate::Ec::DivisionByZero);
        let a = guard_run(&ev, "Order", 0, 4096, 257, false, true, true);
        let b = guard_run(&ev, "Order", 0, 4096, 257, false, true, false);
        assert_eq!(a, b);
        assert_eq!(b.1, 1);
        assert_eq!(
            b.0.expect_err("depth first"),
            EvalError::resource_limit("finite binding traversal")
        );
        hooks.0.store(0, Ordering::SeqCst);
        assert!(
            guard_run(&ev, "Bad", 0, 4096, 0, false, true, false)
                .0
                .is_err()
        );
        assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
        assert_eq!(ev.take_output(), Vec::<String>::new());
    }
}

#[test]
fn tuple_fallback_keeps_late_binding_failure_ahead_of_first_body_error() {
    for options in [Options::dive(), Options::tlc()] {
        let (ev, _) = evaluator(options);
        let (scope, expr) = scope(&ev, "LateTuple");
        let Expr::Quant { bounds, .. } = ev.ir().expr(expr) else {
            panic!("quantifier")
        };
        let value = ev
            .eval_scoped(&scope, bounds[0].domain.expect("domain"), &St::constant())
            .expect("explicit set");
        let dive_value::Value::Set(set) = value else {
            panic!("set")
        };
        let lengths: Vec<_> = set
            .elements(ev.options().ctx)
            .expect("elements")
            .iter()
            .map(|v| match v {
                dive_value::Value::Fcn(f) => f.as_tuple().expect("tuple").len(),
                _ => panic!("tuple"),
            })
            .collect();
        assert_eq!(
            lengths,
            vec![2, 3],
            "the malformed binding must actually be later"
        );
        let old = ev
            .scope_bindings_bounded(&scope, expr, &St::constant(), 4096)
            .expect_err("late tuple binding");
        let mut bodies = 0;
        let actual = ev
            .for_each_scalar_quantifier_scope(&scope, expr, &St::constant(), 4096, |_| {
                bodies += 1;
                Err(EvalError::general("first body marker"))
            })
            .expect_err("eager fallback completes bindings first");
        assert_eq!(actual, old);
        assert_eq!(actual.class(), ErrorClass::Unsupported);
        assert_eq!(bodies, 0);
        assert_eq!(ev.take_output(), Vec::<String>::new());
    }
}

#[test]
fn checked_cartesian_order_and_all_domain_preparation_precede_any_body() {
    // The late zero is not allowed to erase an earlier checked usize overflow.
    let names = (0..65)
        .map(|i| format!("a{i}"))
        .collect::<Vec<_>>()
        .join(",");
    let source = format!(
        "---- MODULE CursorCounts ----\nEXTENDS Integers\nLateEmpty == \\A {names} \\in {{0,1}}, z \\in {{}}: TRUE\nEarlyEmpty == \\A z \\in {{}}, {names} \\in {{0,1}}: TRUE\nLateError == \\A {names} \\in {{0,1}}, z \\in 1..(1 \\div 0): TRUE\n====\n"
    );
    for options in [Options::dive(), Options::tlc()] {
        let a =
            dive_sema::analyze_source("CursorCounts.tla", &source, &dive_sema::Options::default());
        assert!(a.accepted(), "{a:?}");
        let ev = Evaluator::new(
            crate::Model::new(Arc::new(dive_ir::lower(&a).expect("IR"))),
            options,
        )
        .expect("evaluator");
        for name in ["LateEmpty", "EarlyEmpty", "LateError"] {
            let (scope, expr) = scope(&ev, name);
            let eager = ev.scope_bindings_bounded(&scope, expr, &St::constant(), 1000);
            let mut bodies = 0;
            let cursor =
                ev.for_each_scalar_quantifier_scope(&scope, expr, &St::constant(), 1000, |_| {
                    bodies += 1;
                    Ok(())
                });
            assert_eq!(cursor, eager.as_ref().map(|_| ()).map_err(Clone::clone));
            assert_eq!(bodies, 0);
            match name {
                "LateEmpty" => assert_eq!(
                    cursor.expect_err("overflow before late zero"),
                    EvalError::resource_limit("quantifier bindings")
                ),
                "EarlyEmpty" => assert_eq!(eager.expect("early zero prevents overflow").len(), 0),
                "LateError" => assert_eq!(
                    cursor
                        .expect_err("all domains evaluated before Cartesian overflow")
                        .class(),
                    ErrorClass::Eval
                ),
                _ => panic!("fixed test case"),
            }
        }
    }
}

#[test]
fn cursor_refuses_live_and_expired_foreign_contexts_before_domains_or_bodies() {
    for options in [Options::dive(), Options::tlc()] {
        let (ev, hooks) = evaluator(options);
        let mut model = crate::Model::new(ev.ir().clone());
        model.names = ev.core.names.clone();
        let foreign = Evaluator::new(model, options).expect("same IR/names, different core");
        let (scope, expr) = scope(&foreign, "Order");
        assert!(Arc::ptr_eq(&scope.owner, ev.ir()));
        assert!(Arc::ptr_eq(&scope.names, &ev.core.names));
        let mut owner = Some(foreign);
        for expired in [false, true] {
            assert_eq!(scope.context.upgrade().is_none(), expired);
            let old = ev
                .scope_bindings_bounded(&scope, expr, &St::constant(), 4096)
                .expect_err("foreign eager context");
            let mut bodies = 0;
            let new = ev
                .for_each_scalar_quantifier_scope(&scope, expr, &St::constant(), 4096, |_| {
                    bodies += 1;
                    Ok(())
                })
                .expect_err("foreign cursor context");
            assert_eq!(new, old);
            assert_eq!(bodies, 0);
            assert_eq!(new.class(), ErrorClass::Unsupported);
            assert_eq!(hooks.0.load(Ordering::SeqCst), 0);
            assert_eq!(ev.take_output(), Vec::<String>::new());
            if !expired {
                drop(owner.take());
            }
        }
    }
}
