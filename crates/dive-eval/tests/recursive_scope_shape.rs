//! Source-first controls for the existing recursive function and lexical scopes.
#![allow(clippy::expect_used, clippy::panic, reason = "tiny shape assertions")]

use dive_eval::{Evaluator, Model, Options, SlotView, St};
use dive_ir::{DefScope, Expr, LocalRef, SlotKind};
use dive_value::{Ctx, Value};
use std::{path::PathBuf, sync::Arc};

fn evaluator(options: Options) -> Evaluator {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../dive-liveness/tests/fixtures/recursive-function/PermSeqsScope.tla");
    let analysis =
        dive_sema::analyze_file(&path, &dive_sema::Options::default()).expect("read fixture");
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("lower fixture"));
    assert_eq!(dive_ir::validate::check(&ir), [] as [String; 0]);
    Evaluator::new(Model::new(ir), options).expect("owned evaluator")
}

#[test]
fn original_recursive_ir_keeps_self_formal_domain_and_let_capture() {
    let ev = evaluator(Options::tlc());
    let ir = ev.ir();
    let def = ir.root_def("PermSeqs").expect("PermSeqs");
    let root = ir.def(def);
    let Expr::Let { defs, .. } = ir.expr(root.body) else {
        panic!("outer LET");
    };
    let local = defs
        .iter()
        .copied()
        .find(|d| ir.def(*d).name == "perms")
        .expect("perms");
    let DefScope::LetValue { slot, .. } = ir.def(local).scope else {
        panic!("lazy LET value");
    };
    let rec = ir.def(local).body;
    let Expr::RecFcn { name, bounds, body } = ir.expr(rec) else {
        panic!("recursive function");
    };
    assert_eq!(root.frame[name.index()].kind, SlotKind::FcnName);
    assert_eq!(bounds.len(), 1);
    assert_eq!(bounds[0].vars.len(), 1);
    assert_eq!(root.frame[bounds[0].vars[0].index()].kind, SlotKind::Bound);
    assert!(bounds[0].domain.is_some());
    assert!(matches!(ir.expr(*body), Expr::If { .. }));
    let captured = Value::set_raw(vec![Value::Int(1), Value::Int(2)], Ctx::tlc());
    let scope = ev
        .scope_call_values(
            &ev.scope(def).expect("root scope"),
            def,
            vec![captured.clone()],
        )
        .expect("call scope");
    let entered = ev.scope_let(&scope, root.body).expect("LET scope");
    let SlotView::Lazy(actual, lexical) = ev.scope_slot(&entered, LocalRef { up: 0, slot }) else {
        panic!("original thunk");
    };
    assert_eq!(actual, rec);
    assert!(
        matches!(ev.scope_slot(&lexical, LocalRef { up: 0, slot: dive_ir::Slot(0) }), SlotView::Value(value) if value == captured)
    );
}

#[test]
fn actual_recursive_evaluation_reuses_no_other_invocations_capture() {
    for options in [Options::tlc(), Options::dive()] {
        let ev = evaluator(options);
        let def = ev.ir().root_def("PermSeqs").expect("PermSeqs");
        let body = ev.ir().def(def).body;
        let root = ev.scope(def).expect("root scope");
        for values in [vec![], vec![1], vec![1, 2], vec![3], vec![1, 2], vec![]] {
            let input =
                Value::set_raw(values.iter().copied().map(Value::Int).collect(), Ctx::tlc());
            let scope = ev.scope_call_values(&root, def, vec![input]).expect("call");
            let expected = match values.as_slice() {
                [] => vec![Value::tuple(vec![])],
                [a] => vec![Value::tuple(vec![Value::Int(*a)])],
                [a, b] => vec![
                    Value::tuple(vec![Value::Int(*a), Value::Int(*b)]),
                    Value::tuple(vec![Value::Int(*b), Value::Int(*a)]),
                ],
                _ => panic!("tiny controls"),
            };
            let expected = Value::set(expected, Ctx::tlc()).expect("expected finite relation");
            for _ in 0..2 {
                assert!(
                    dive_value::tlc::equals(
                        &ev.eval_scoped(&scope, body, &St::constant())
                            .expect("existing recursion"),
                        &expected,
                        Ctx::tlc()
                    )
                    .expect("finite equality")
                );
            }
        }
    }
}

#[test]
fn projected_body_contexts_keep_actual_self_and_reject_foreign_or_stale_invocations() {
    let ev = evaluator(Options::tlc());
    let def = ev.ir().root_def("PermSeqs").expect("PermSeqs");
    let root = &ev.ir().def(def);
    let Expr::Let { defs, .. } = ev.ir().expr(root.body) else {
        panic!("LET");
    };
    let local = defs
        .iter()
        .copied()
        .find(|d| ev.ir().def(*d).name == "perms")
        .expect("perms");
    let rec = ev.ir().def(local).body;
    let Expr::RecFcn { name, bounds, .. } = ev.ir().expr(rec) else {
        panic!("recursive function");
    };
    let domain = Value::set(vec![1.into(), 2.into()], Ctx::tlc()).expect("domain");
    let scope = ev
        .scope_call_values(&ev.scope(def).expect("scope"), def, vec![domain])
        .expect("call");
    let scope = ev.scope_let(&scope, root.body).expect("LET");
    let invocation = ev
        .begin_pure_predicate(&scope, &St::constant(), 4096)
        .expect("invocation");
    let function = ev
        .scope_recursive_function_bounded(&scope, rec, &invocation, 4096)
        .expect("actual function");
    let contexts = ev
        .recursive_function_body_scopes_bounded(&scope, &function, &invocation, 4096)
        .expect("contexts");
    assert_eq!(contexts.bindings.len(), 4);
    for bound in &contexts.bindings {
        assert!(
            matches!(ev.scope_slot(bound,LocalRef{up:0,slot:*name}),SlotView::Function(actual) if Arc::ptr_eq(&actual,&function))
        );
        assert!(matches!(
            ev.scope_slot(
                bound,
                LocalRef {
                    up: 0,
                    slot: bounds[0].vars[0]
                }
            ),
            SlotView::Value(_)
        ));
        ev.eval_scoped(bound, contexts.body, &contexts.state)
            .expect("real recursive body");
    }
    let other = evaluator(Options::tlc());
    let other_def = other.ir().root_def("PermSeqs").expect("other");
    let foreign_invocation = other
        .begin_pure_predicate(
            &other.scope(other_def).expect("other scope"),
            &St::constant(),
            4096,
        )
        .expect("foreign invocation");
    assert!(
        ev.scope_recursive_function_bounded(&scope, rec, &foreign_invocation, 4096)
            .is_err()
    );
    assert!(
        ev.scope_recursive_function_bounded(&scope, dive_ir::ExprId(u32::MAX), &invocation, 4096)
            .is_err()
    );
    assert!(
        other
            .recursive_function_body_scopes_bounded(
                &other.scope(other_def).expect("scope"),
                &function,
                &invocation,
                4096
            )
            .is_err()
    );
    let stale = ev
        .begin_pure_predicate(&scope, &St::constant(), 4096)
        .expect("different invocation");
    assert!(
        ev.recursive_function_body_scopes_bounded(&scope, &function, &stale, 4096)
            .is_err()
    );
    let changed = invocation.at_state(Arc::from([Some(1.into()), Some(false.into())]));
    assert!(
        ev.recursive_function_body_scopes_bounded(&scope, &function, &changed, 4096)
            .is_err()
    );
    for limit in [0, 1] {
        assert!(
            ev.scope_recursive_function_bounded(&scope, rec, &invocation, limit)
                .is_err()
        );
    }
}

fn source_evaluator(body: &str) -> Evaluator {
    let source = format!("---- MODULE RecursiveSmall ----\nEXTENDS Integers, TLC\n{body}\n====\n");
    let analysis = dive_sema::analyze_source(
        "RecursiveSmall.tla",
        &source,
        &dive_sema::Options::default(),
    );
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    Evaluator::new(
        Model::new(Arc::new(dive_ir::lower(&analysis).expect("IR"))),
        Options {
            max_depth: 64,
            ..Options::tlc()
        },
    )
    .expect("evaluator")
}
#[test]
fn empty_unused_domains_and_nonterminating_or_invalid_applications_keep_boundaries() {
    let ev = source_evaluator(
        "Empty==LET f[x \\in {}]==IF x=0 THEN 1 \\div 0 ELSE f[x] IN DOMAIN f\nUnused==LET f[x \\in 1 \\div 0]==f[x] IN TRUE\nLoop==LET f[x \\in {0}]==f[x] IN f[0]\nOutside==LET f[x \\in {0}]==IF x=0 THEN 7 ELSE f[x] IN f[1]\nInfinite==LET f[x \\in Nat]==f[x] IN DOMAIN f\nLarge==LET f[x \\in 1..1000000]==f[x] IN DOMAIN f\nTuple==LET f[<<x,y>> \\in {<<0,1>>}]==IF x=0 THEN y ELSE f[<<0,y>>] IN f[<<0,1>>]\nMultiple==LET f[x \\in {0},y \\in {1}]==IF x=0 THEN y ELSE f[0,y] IN f[0,1]",
    );
    for name in ["Empty", "Unused", "Tuple", "Multiple"] {
        let def = ev.ir().root_def(name).expect(name);
        let scope = ev.scope(def).expect("scope");
        ev.check_finite_bindings(&scope, ev.ir().def(def).body, &St::constant(), 4096)
            .expect(name);
        let value = ev.eval_def(def, &St::constant()).expect(name);
        match name {
            "Empty" => assert_eq!(value, Value::empty_set(Ctx::tlc())),
            "Unused" => assert_eq!(value, Value::Bool(true)),
            _ => assert_eq!(value, Value::Int(1)),
        }
    }
    for name in ["Infinite", "Large"] {
        let def = ev.ir().root_def(name).expect(name);
        assert!(
            ev.check_finite_bindings(
                &ev.scope(def).expect("scope"),
                ev.ir().def(def).body,
                &St::constant(),
                4096
            )
            .is_err(),
            "{name}"
        );
    }
    for name in ["Loop", "Outside"] {
        let def = ev.ir().root_def(name).expect(name);
        let error = ev
            .eval_def(def, &St::constant())
            .expect_err("original recursive evaluation error");
        if name == "Loop" {
            assert_eq!(error.class(), dive_eval::ErrorClass::ResourceLimit);
            assert!(error.message().contains("64 levels deep"));
        } else {
            assert_eq!(error.class(), dive_eval::ErrorClass::Eval);
            assert!(error.message().contains("not in its domain"));
        }
        assert!(
            error.origin().is_some(),
            "original source location: {error}"
        );
    }
}
#[test]
fn recursive_effects_are_inspected_before_any_runtime_host_callback() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[derive(Debug, Default)]
    struct Host(AtomicUsize);
    impl dive_eval::Host for Host {
        fn print_output(&self, _: &str) -> Result<bool, dive_eval::HostError> {
            self.0.fetch_add(1, Ordering::Relaxed);
            Ok(true)
        }
    }
    let ev = source_evaluator(
        "Bad==LET f[x \\in {0}]==IF PrintT(x) THEN f[x] ELSE FALSE IN f[0]\nCaptured(p)==LET f[x \\in {0}]==IF x=0 THEN p ELSE f[x] IN f[0]\nHidden==Captured(PrintT(0))\nEmptyBad==LET f[x \\in {}]==IF PrintT(x) THEN f[x] ELSE FALSE IN DOMAIN f",
    );
    let host = Arc::new(Host::default());
    ev.set_host(host.clone());
    for name in ["Bad", "Hidden", "EmptyBad"] {
        let def = ev.ir().root_def(name).expect(name);
        assert!(
            ev.check_finite_bindings(
                &ev.scope(def).expect("scope"),
                ev.ir().def(def).body,
                &St::constant(),
                4096
            )
            .is_err(),
            "{name}"
        );
    }
    assert_eq!(host.0.load(Ordering::Relaxed), 0);
}

#[test]
fn actual_recursive_body_cannot_capture_a_primed_or_hidden_primed_value() {
    let ev = source_evaluator(
        "VARIABLE v\nDirect==LET f[t \\in {0}]==IF t=0 THEN v' ELSE f[t] IN f[0]\nF(p)==LET f[t \\in {0}]==IF t=0 THEN p ELSE f[t] IN f[0]\nHidden==F(v')",
    );
    let state = St::action(Arc::from([Some(0.into())]), Arc::from([Some(1.into())]));
    for name in ["Direct", "Hidden"] {
        let def = ev.ir().root_def(name).expect(name);
        let error = ev
            .check_finite_bindings(
                &ev.scope(def).expect("scope"),
                ev.ir().def(def).body,
                &state,
                4096,
            )
            .expect_err("recursive action capture remains refused");
        assert!(error.message().contains("recursive"), "{name}: {error}");
    }
}
