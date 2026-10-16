//! Name-state ownership, lexical cutoff and evaluator execution regressions.
#![allow(
    clippy::expect_used,
    reason = "test setup and checked fixture assertions"
)]

use std::path::Path;
use std::sync::Arc;

use dive_ir::{Arg, DeclId, ExprId, Pass};
use dive_value::{Interner, Value};

use crate::env::{Sv, Thunk};
use crate::{Evaluator, Model, Options, SlotView, St, StateRef};

fn evaluator(options: Options) -> Evaluator {
    let text = "---- MODULE NamedQueries ----\nEXTENDS TLC\nVARIABLES x,y\nCONSTANT C\nRead == x\nReadY == y\nAfter == x'\nSame == UNCHANGED x\nLiteral == TRUE\nPrinted == Print(x,x)\nSide == Print(42,TRUE)\nBindings == \\E i \\in Print(42,{1}): TRUE\nR[i \\in {1}] == IF i=1 THEN TRUE ELSE R[1]\nWrap(v) == v\nNested == ENABLED (x'=TRUE /\\ ENABLED (x'=FALSE) /\\ x'=TRUE)\nNestedConflict == ENABLED (x'=TRUE /\\ ENABLED (x'=FALSE /\\ x'=TRUE))\nPrimedNested == ENABLED (x'=TRUE /\\ (ENABLED (x'=x))')\nPrimedMissing == ENABLED (x'=TRUE /\\ (ENABLED (y'=y))')\n====\n";
    let analysis =
        dive_sema::analyze_source("NamedQueries.tla", text, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    Evaluator::new(Model::new(ir), options).expect("evaluator")
}

fn decl(ev: &Evaluator, name: &str) -> DeclId {
    ev.ir()
        .decls()
        .iter()
        .enumerate()
        .find_map(|(i, d)| (d.name == name).then(|| DeclId(u32::try_from(i).expect("small IR"))))
        .expect("declaration")
}

fn body(ev: &Evaluator, name: &str) -> ExprId {
    ev.ir()
        .def(ev.ir().root_def(name).expect("definition"))
        .body
}

fn named(ev: &Evaluator, values: &[(&str, bool)]) -> Arc<crate::NamedState> {
    ev.named_state(
        values
            .iter()
            .map(|(name, value)| (decl(ev, name), Value::Bool(*value))),
    )
    .expect("named state")
}

#[test]
fn public_named_states_preserve_root_reads_prime_and_undefined_assignments() {
    for options in [Options::tlc(), Options::default()] {
        let ev = evaluator(options);
        let current = named(&ev, &[("x", false), ("y", true)]);
        let next = named(&ev, &[("x", true)]);
        let state = St {
            s0: StateRef::Named(current.clone()),
            s1: StateRef::Named(next.clone()),
            ..St::empty()
        };
        assert_eq!(
            ev.eval_in(
                ev.ir().root_def("Read").expect("read"),
                body(&ev, "Read"),
                &state
            )
            .expect("current"),
            Value::FALSE
        );
        assert_eq!(
            ev.eval_def(ev.ir().root_def("After").expect("after"), &state)
                .expect("next"),
            Value::TRUE
        );
        assert_eq!(
            ev.eval_def(ev.ir().root_def("Same").expect("same"), &state)
                .expect("changed"),
            Value::FALSE
        );
        let only_next = state.primed_successor();
        assert!(
            ev.eval_def(ev.ir().root_def("ReadY").expect("y"), &only_next)
                .is_err(),
            "missing named y must not fall back to the complete old state"
        );
        assert_eq!(current.len(), 2);
        assert_eq!(
            next.assignments().collect::<Vec<_>>(),
            vec![("x", &Value::TRUE)]
        );
        assert_eq!(
            ev.ir().vars().len(),
            2,
            "query assignments never add graph variables"
        );
    }
}

#[test]
fn named_construction_and_basic_evaluation_reject_foreign_contexts() {
    let ev = evaluator(Options::tlc());
    let foreign = Evaluator::new(Model::new(ev.ir().clone()), Options::tlc()).expect("other core");
    let x = decl(&ev, "x");
    assert!(ev.named_state([(DeclId(u32::MAX), Value::TRUE)]).is_err());
    assert!(ev.named_state([(decl(&ev, "C"), Value::TRUE)]).is_err());
    assert!(
        ev.named_state([(x, Value::TRUE), (x, Value::FALSE)])
            .is_err()
    );
    assert_eq!(
        ev.named_state([(x, Value::TRUE), (x, Value::TRUE)])
            .expect("same name")
            .len(),
        1
    );
    let alien = Value::string(&Interner::new(), "not interned here");
    assert!(ev.named_state([(x, alien)]).is_err());

    let bad = St {
        s0: StateRef::Named(named(&foreign, &[("x", true)])),
        ..St::empty()
    };
    let literal = ev.ir().root_def("Literal").expect("literal");
    let scope = ev.scope(literal).expect("scope");
    for error in [
        ev.eval_def(literal, &bad).expect_err("foreign state"),
        ev.eval_in(literal, body(&ev, "Literal"), &bad)
            .expect_err("foreign state"),
        ev.eval_scoped(&scope, body(&ev, "Literal"), &bad)
            .expect_err("foreign state"),
        ev.begin_pure_predicate(&scope, &bad, 8)
            .expect_err("foreign state"),
        ev.scope_substitution(&scope, x, &bad, false)
            .expect_err("foreign state"),
    ] {
        assert!(error.message().contains("C004"), "{error}");
    }
}

#[test]
fn actual_warm_thunk_reuses_compatible_names_and_observes_changed_candidates() {
    let ev = evaluator(Options::tlc());
    let printed = ev.ir().root_def("Printed").expect("printed");
    let scope = ev.scope(printed).expect("scope");
    let slot = Sv::Thunk(Thunk::new(body(&ev, "Printed"), scope.env.clone(), true));
    let empty = StateRef::Named(named(&ev, &[]));
    let first = St {
        s0: StateRef::Named(named(&ev, &[("x", false)])),
        s1: empty,
        ..St::empty()
    };
    let force = |st: &St| {
        let value = ev
            .core
            .force(&mut scope.env.clone(), &slot, st, body(&ev, "Printed"))
            .expect("force");
        ev.core.force_fcn(value).expect("scalar value")
    };
    assert_eq!(force(&first), Value::FALSE);
    assert_eq!(ev.take_output().len(), 1);
    let extension = St {
        s0: StateRef::Named(named(&ev, &[("x", false), ("y", true)])),
        ..first.clone()
    };
    assert_eq!(force(&extension), Value::FALSE);
    assert!(
        ev.take_output().is_empty(),
        "compatible named assignment reuses the actual cache"
    );
    let changed = St {
        s0: StateRef::Named(named(&ev, &[("x", true), ("y", true)])),
        ..first.clone()
    };
    assert_eq!(force(&changed), Value::TRUE);
    assert_eq!(ev.take_output().len(), 1);
    let indexed = St::state(Arc::from([Some(Value::FALSE), Some(Value::TRUE)]));
    assert_eq!(force(&indexed), Value::FALSE);
    assert_eq!(
        ev.take_output().len(),
        1,
        "indexed and named states never establish mixed cache inclusion"
    );
    assert!(!first.s0.includes(&indexed.s0));
    assert!(!indexed.s0.includes(&first.s0));
}

#[test]
fn query_state_projections_keep_whole_successor_and_one_shared_work_budget() {
    let ev = evaluator(Options::tlc());
    let literal = ev.ir().root_def("Literal").expect("literal");
    let scope = ev.scope(literal).expect("scope");
    let base = ev
        .begin_pure_predicate(
            &scope,
            &St::state(Arc::from([Some(Value::FALSE), Some(Value::TRUE)])),
            17,
        )
        .expect("budget");
    let successor = named(&ev, &[("x", true), ("y", false)]);
    let candidate = base.enabled_candidate(successor.clone());
    let current = candidate.enabled_current();
    let primed = candidate.primed_successor();
    for state in [&candidate, &current, &primed] {
        assert!(Arc::ptr_eq(
            state.selectseq.as_ref().expect("budget"),
            base.selectseq.as_ref().expect("budget")
        ));
    }
    assert!(matches!(&primed.s0, StateRef::Named(actual) if Arc::ptr_eq(actual, &successor)));
    assert!(matches!(primed.s1, StateRef::Null));
    assert!(primed.is(crate::env::control::PRIMED));
    assert!(candidate.is(crate::env::control::ENABLED));
    assert_eq!(successor.get("y"), Some(&Value::FALSE));
}

#[test]
fn substitution_projection_distinguishes_unset_absent_and_captured_cutoffs() {
    let ev = evaluator(Options::tlc());
    let read = ev.ir().root_def("Read").expect("read");
    let mut scope = ev.scope(read).expect("scope");
    let x = decl(&ev, "x");
    let c = decl(&ev, "C");
    scope.env.dynamic.push(x, Sv::Unset);
    scope.env.dynamic.push(c, Sv::Unset);
    let branched = ev.scope_enabled(&scope).expect("branch");
    let state = St::state(Arc::from([Some(Value::FALSE), Some(Value::TRUE)]))
        .enabled_candidate(named(&ev, &[]));
    assert!(matches!(
        ev.scope_substitution(&branched, x, &state, false)
            .expect("lookup"),
        Some(SlotView::Unset)
    ));
    assert!(
        ev.scope_substitution(&branched, x, &state, true)
            .expect("cut")
            .is_none()
    );
    assert!(matches!(
        ev.scope_substitution(&branched, c, &state, true)
            .expect("constant"),
        Some(SlotView::Unset)
    ));
    assert!(matches!(ev.scope_subst_slot(&branched, x), SlotView::Unset));
    let mut fresh = branched.clone();
    fresh.env.dynamic.push(x, Sv::Val(Value::TRUE));
    assert!(matches!(
        ev.scope_substitution(&fresh, x, &state, true)
            .expect("new link"),
        Some(SlotView::Value(Value::Bool(true)))
    ));
    assert!(
        ev.scope_substitution(&branched, x, &state, true)
            .expect("old branch")
            .is_none()
    );
    assert!(matches!(
        ev.scope_substitution(&scope, x, &state, true)
            .expect("captured prebranch"),
        Some(SlotView::Unset)
    ));
    assert!(
        ev.scope_substitution(&scope, DeclId(u32::MAX), &state, false)
            .is_err()
    );
}

#[cfg(feature = "enumerate")]
#[test]
fn nested_enabled_searches_use_separate_candidate_namespaces() {
    for options in [Options::tlc(), Options::default()] {
        let ev = evaluator(options);
        let state = St::state(Arc::from([Some(Value::FALSE), Some(Value::TRUE)]));
        assert_eq!(
            ev.eval_def(ev.ir().root_def("Nested").expect("nested"), &state)
                .expect("nested query"),
            Value::TRUE
        );
        assert_eq!(
            ev.eval_def(
                ev.ir().root_def("NestedConflict").expect("conflict"),
                &state
            )
            .expect("nested conflict"),
            Value::FALSE
        );
    }
}

#[cfg(feature = "enumerate")]
#[test]
fn committed_primary_namespace_cases_match_query_truth_in_both_profiles() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../conformance/corpus/instance-temporal/specifications");
    let cases = [
        ("conflict_decoy_exported", true),
        ("conflict_no_root_exported", true),
        ("conflict_no_root_nested", false),
        ("conflict_same_direct", false),
        ("conflict_same_exported", true),
        ("conflict_same_outside", false),
        ("guard_decoy_exported", true),
        ("guard_decoy_nested", true),
        ("rebind_no_root_fresh", false),
        ("rebind_no_root_old", true),
    ];
    for options in [Options::tlc(), Options::default()] {
        for (name, expected) in cases {
            let path = corpus.join(name).join("Probe.tla");
            let analysis =
                dive_sema::analyze_file(&path, &dive_sema::Options::default()).expect("fixture");
            assert!(analysis.accepted(), "{name}: {:?}", analysis.diagnostics());
            let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
            let ev = Evaluator::new(Model::new(ir), options).expect("evaluator");
            let values: Arc<[Option<Value>]> = ev
                .ir()
                .vars()
                .iter()
                .map(|decl| {
                    Some(Value::Bool(
                        name.starts_with("guard_") && ev.ir().decl(*decl).name == "y"
                            || name.starts_with("conflict_decoy")
                                && ev.ir().decl(*decl).name == "y",
                    ))
                })
                .collect();
            let before = ev.ir().vars().len();
            let q = ev.ir().root_def("Q").expect("query");
            assert_eq!(
                ev.eval_def(q, &St::state(values.clone())).expect(name),
                Value::Bool(expected),
                "{name}"
            );
            if name.starts_with("guard_") {
                let mut after = values.to_vec();
                let x = ev
                    .ir()
                    .vars()
                    .iter()
                    .position(|decl| ev.ir().decl(*decl).name == "x")
                    .expect("x");
                after[x] = Some(Value::TRUE);
                assert_eq!(
                    ev.eval_def(q, &St::state(after.into()))
                        .expect("guard after"),
                    Value::FALSE,
                    "{name}"
                );
            }
            assert_eq!(ev.ir().vars().len(), before);
        }
    }
}

#[test]
fn distinct_variable_declarations_with_the_same_name_share_one_assignment() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(
        "../../conformance/corpus/instance-temporal/specifications/conflict_same_exported/Probe.tla");
    let analysis = dive_sema::analyze_file(&path, &dive_sema::Options::default()).expect("fixture");
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let ev = Evaluator::new(Model::new(ir), Options::tlc()).expect("evaluator");
    let ys: Vec<DeclId> = ev
        .ir()
        .decls()
        .iter()
        .enumerate()
        .filter(|(_, d)| d.variable && d.name == "y")
        .map(|(i, _)| DeclId(u32::try_from(i).expect("small IR")))
        .collect();
    assert!(
        ys.len() >= 2,
        "root and imported y are distinct declarations"
    );
    assert_ne!(ys[0], ys[1]);
    let before = ev.core.names.len();
    let state = ev
        .named_state([(ys[0], Value::TRUE), (ys[1], Value::TRUE)])
        .expect("same canonical name");
    assert_eq!(state.len(), 1);
    assert_eq!(state.get("y"), Some(&Value::TRUE));
    assert!(
        ev.named_state([(ys[0], Value::TRUE), (ys[1], Value::FALSE)])
            .is_err()
    );
    assert_eq!(
        ev.core.names.len(),
        before,
        "query keys do not intern TLA+ value names"
    );
}

#[cfg(feature = "enumerate")]
#[test]
fn primed_nested_query_keeps_partial_named_current_and_missing_values_are_errors() {
    for options in [Options::tlc(), Options::default()] {
        let ev = evaluator(options);
        let state = St::state(Arc::from([Some(Value::FALSE), Some(Value::TRUE)]));
        assert_eq!(
            ev.eval_def(
                ev.ir().root_def("PrimedNested").expect("primed nested"),
                &state
            )
            .expect("whole named successor"),
            Value::TRUE
        );
        let error = ev
            .eval_def(ev.ir().root_def("PrimedMissing").expect("missing"), &state)
            .expect_err("outer successor has no y");
        assert!(error.message().contains('y'), "{error}");
        assert_eq!(error.ec().code(), 2280);
    }
}

fn foreign_states(ev: &Evaluator, foreign: &Evaluator) -> [St; 2] {
    let local = named(ev, &[("x", false), ("y", true)]);
    let alien = named(foreign, &[("x", true)]);
    [
        St {
            s0: StateRef::Named(alien.clone()),
            s1: StateRef::Named(local.clone()),
            ..St::empty()
        },
        St {
            s0: StateRef::Named(local),
            s1: StateRef::Named(alien),
            ..St::empty()
        },
    ]
}

#[test]
fn state_entries_reject_foreign_named_before_callbacks_and_captures() {
    for options in [Options::tlc(), Options::default()] {
        let ev = evaluator(options);
        let foreign = evaluator(options);
        let side = ev.ir().root_def("Side").expect("side");
        let scope = ev.scope(side).expect("scope");
        let bindings = ev
            .scope(ev.ir().root_def("Bindings").expect("binder"))
            .expect("scope");
        let recursive = ev
            .scope(ev.ir().root_def("R").expect("recursive"))
            .expect("scope");
        let local = St::state(Arc::from([Some(Value::FALSE), Some(Value::TRUE)]));
        let function = ev
            .scope_recursive_function_bounded(&recursive, body(&ev, "R"), &local, 4096)
            .expect("local capture");
        let arg = Arg::Expr {
            expr: body(&ev, "Side"),
            pass: Pass::Lazy,
        };
        for bad in foreign_states(&ev, &foreign) {
            for error in [
                ev.eval_scoped(&scope, body(&ev, "Side"), &bad)
                    .expect_err("foreign callback"),
                ev.eval_arg(&scope, &arg, &bad)
                    .expect_err("foreign argument"),
                ev.scope_bindings(&bindings, body(&ev, "Bindings"), &bad)
                    .expect_err("foreign domain"),
                ev.scope_bindings_bounded(&bindings, body(&ev, "Bindings"), &bad, 4096)
                    .expect_err("foreign bounded domain"),
                ev.scope_recursive_function_bounded(&recursive, body(&ev, "R"), &bad, 4096)
                    .expect_err("foreign capture"),
                ev.recursive_function_body_scopes_bounded(&recursive, &function, &bad, 4096)
                    .expect_err("foreign invocation"),
                ev.check_finite_bindings(&scope, body(&ev, "Side"), &bad, 4096)
                    .expect_err("foreign finite input"),
                ev.check_finite_data_bindings(&scope, body(&ev, "Side"), &bad, 4096)
                    .expect_err("foreign finite data"),
                ev.eval_scoped_pure(&scope, body(&ev, "Side"), &bad, 4096)
                    .expect_err("foreign pure input"),
                ev.random_element_member(&scope, body(&ev, "Side"), &bad, &Value::TRUE, 4096)
                    .expect_err("foreign relation input"),
            ] {
                assert!(error.message().contains("C004"), "{error}");
            }
            assert!(
                ev.take_output().is_empty(),
                "no callback before context rejection"
            );
            assert!(!ev.has_print_event());
        }
        assert_eq!(
            ev.eval_arg(&scope, &arg, &local).expect("local callback"),
            Value::TRUE
        );
        assert_eq!(ev.take_output(), ["42  TRUE"]);
        assert_eq!(
            ev.scope_bindings(&bindings, body(&ev, "Bindings"), &local)
                .expect("local binder")
                .len(),
            1
        );
        assert_eq!(ev.take_output(), ["42  {1}"]);
        assert_eq!(
            ev.recursive_function_body_scopes_bounded(&recursive, &function, &local, 4096)
                .expect("local body projection")
                .bindings
                .len(),
            1
        );
    }
}

#[cfg(feature = "enumerate")]
#[test]
fn action_and_reused_query_budget_reject_both_foreign_named_states() {
    for options in [Options::tlc(), Options::default()] {
        let ev = evaluator(options);
        let foreign = evaluator(options);
        let side = ev.ir().root_def("Side").expect("side");
        let action = ev.init_predicate(side, 0).expect("action");
        let literal = ev.ir().root_def("Literal").expect("literal");
        let scope = ev.scope(literal).expect("scope");
        let local = St::state(Arc::from([Some(Value::FALSE), Some(Value::TRUE)]));
        let budget = ev
            .begin_pure_predicate(&scope, &local, 4096)
            .expect("local invocation");
        for bad in foreign_states(&ev, &foreign) {
            let bounded = St {
                s0: bad.s0.clone(),
                s1: bad.s1.clone(),
                ..budget.clone()
            };
            for error in [
                ev.eval_action(&action, &bad).expect_err("foreign action"),
                ev.enabled_scoped_pure(&scope, body(&ev, "Literal"), None, &bad, 4096)
                    .expect_err("foreign fresh query"),
                ev.enabled_scoped_pure(&scope, body(&ev, "Literal"), None, &bounded, 4096)
                    .expect_err("foreign query with existing local budget"),
                ev.eval_scoped_pure(&scope, body(&ev, "Literal"), &bounded, 4096)
                    .expect_err("foreign pure evaluation with existing local budget"),
            ] {
                assert!(error.message().contains("C004"), "{error}");
            }
            assert_eq!(ev.take_output(), Vec::<String>::new());
            assert!(!ev.has_print_event());
        }
        assert_eq!(
            ev.eval_action(&action, &local).expect("local action"),
            Value::TRUE
        );
        assert_eq!(ev.take_output(), ["42  TRUE"]);
        assert!(
            ev.enabled_scoped_pure(&scope, body(&ev, "Literal"), None, &budget, 4096)
                .expect("local query with reused budget")
        );
    }
}
