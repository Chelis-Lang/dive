//! Original-order Init/filter pull cursors, without predicate pruning.
#![allow(
    clippy::expect_used,
    reason = "bounded source fixtures and route assertions"
)]

use dive_eval::next::{Action, Flow};
use dive_eval::{ErrorClass, Evaluator, HostControl, Interrupted, Model, Options, St};
use dive_ir::actions::{ActionName, split};
use dive_value::Value;
use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

struct Fixture {
    ev: Evaluator,
    inits: Vec<Action>,
    actions: Vec<Action>,
    reference: dive_ir::DefId,
}
fn fixture(domain: &str, options: Options) -> Fixture {
    let text = format!(
        "---- MODULE Cursor ----\nEXTENDS Integers, FiniteSets, TLC\nVARIABLE x\nReference == {domain}\nInit == x \\in Reference\nNext == x' \\in Reference\nWitness == x' \\in SUBSET (SUBSET (1..5))\n====\n"
    );
    from_source(&text, options)
}

fn from_source(text: &str, options: Options) -> Fixture {
    let analysis = dive_sema::analyze_source("Cursor.tla", text, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("valid lowered fixture"));
    let init = ir.root_def("Init").expect("Init");
    let next = ir.root_def("Next").expect("Next");
    let reference = ir.root_def("Reference").expect("Reference");
    let ev = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
    let inits = vec![ev.init_predicate(init, 0).expect("initial predicate")];
    let tree = split(
        &ir,
        ir.def(next).body,
        ActionName::Def(next),
        next,
        &std::collections::BTreeMap::new(),
    );
    let actions = ev.actions(&tree, next, 1).expect("next actions");
    Fixture {
        ev,
        inits,
        actions,
        reference,
    }
}

fn set(value: &Value) -> Option<&dive_value::Set> {
    match value {
        Value::Set(set) => Some(set),
        _ => None,
    }
}

#[test]
fn complete_original_predicate_outputs_match_a_fresh_bulk_reference() {
    for options in [Options::dive(), Options::tlc()] {
        for domain in [
            "{A \\in SUBSET (SUBSET (1..2)) : \\A e \\in A : Cardinality(e) = 1}",
            "{<<a,b>> \\in (1..2) \\X (3..4) : a + b > 0}",
            "{B \\in {A \\in SUBSET (SUBSET (1..2)) : \\A e \\in A : Cardinality(e) = 1} : Cardinality(B) <= 1}",
        ] {
            // The original eager provider/kernel runs first, on a fresh Core.
            let reference = fixture(domain, options);
            let value = reference
                .ev
                .eval_def(reference.reference, &St::constant())
                .expect("reference expression");
            let expected = set(&value)
                .expect("set fixture")
                .elements(options.ctx)
                .expect("bounded legacy bulk");
            let actual = fixture(domain, options);
            for initial in [true, false] {
                let mut values = Vec::new();
                let mut sink = |successor: &dive_eval::next::Successor<'_>| {
                    assert_eq!(successor.ordinal, values.len() as u64);
                    values.push(successor.state[0].clone().expect("assigned x"));
                    Flow::Continue
                };
                let result = if initial {
                    actual.ev.init_states(&actual.inits, &mut sink)
                } else {
                    actual
                        .ev
                        .successors(&actual.actions, &Arc::from([Some(0.into())]), &mut sink)
                }
                .expect("complete cursor");
                assert_eq!(values.as_slice(), expected.as_ref(), "{domain}/{initial}");
                assert_eq!(result.emitted, expected.len() as u64);
                assert!(!result.stopped);
            }
        }
    }
}

#[test]
fn first_empty_reaches_init_successor_and_enabled_before_later_errors_or_effects() {
    let domain =
        "{A \\in SUBSET (SUBSET (1..5)) : Print(Cardinality(A), \\A e \\in A : 1 \\div 0 = 0)}";
    for options in [Options::dive(), Options::tlc()] {
        for route in 0..3 {
            let f = fixture(domain, options);
            if route == 2 {
                let next = f.ev.ir().root_def("Next").expect("Next");
                assert!(
                    f.ev.enabled_scoped(
                        &f.ev.scope(next).expect("scope"),
                        f.ev.ir().def(next).body,
                        None,
                        Arc::from([Some(0.into())])
                    )
                    .expect("empty witness")
                );
            } else {
                let mut seen = Vec::new();
                let mut sink = |successor: &dive_eval::next::Successor<'_>| {
                    seen.push(successor.state[0].clone().expect("x"));
                    Flow::Stop
                };
                let result = if route == 0 {
                    f.ev.init_states(&f.inits, &mut sink)
                } else {
                    f.ev.successors(&f.actions, &Arc::from([Some(0.into())]), &mut sink)
                }
                .expect("stop before later predicate");
                assert_eq!(seen, vec![Value::empty_set(options.ctx)]);
                assert_eq!(result.emitted, 1);
                assert!(result.stopped);
            }
            assert_eq!(f.ev.take_output(), ["0  TRUE"]);
        }
    }
}

#[test]
fn nonboolean_and_intrinsic_errors_keep_the_already_emitted_empty_prefix() {
    for options in [Options::dive(), Options::tlc()] {
        for (body, code) in [("1", 2215), ("1 \\div 0 = 0", 2179)] {
            let domain = format!(
                "{{A \\in SUBSET (SUBSET (1..2)) : Print(Cardinality(A), \\A e \\in A : {body})}}"
            );
            let f = fixture(&domain, options);
            let mut values = Vec::new();
            let error =
                f.ev.init_states(&f.inits, &mut |successor| {
                    values.push(successor.state[0].clone().expect("x"));
                    Flow::Continue
                })
                .expect_err("later original failure");
            assert_eq!(values, vec![Value::empty_set(options.ctx)]);
            assert_eq!(error.emitted, 1);
            assert_eq!(error.error.ec().code(), code);
            assert_eq!(f.ev.take_output(), ["0  TRUE"]);
        }
    }
}

#[test]
fn effects_remain_interleaved_with_each_actual_sink_and_nested_filter() {
    for options in [Options::dive(), Options::tlc()] {
        let f = fixture(
            "{A \\in SUBSET (1..2) : Print(Cardinality(A), TRUE)}",
            options,
        );
        let mut observations = Vec::new();
        let result =
            f.ev.init_states(&f.inits, &mut |successor| {
                observations.push((successor.state[0].clone().expect("x"), f.ev.take_output()));
                Flow::Continue
            })
            .expect("four ordered subsets");
        assert_eq!(result.emitted, 4);
        assert_eq!(
            observations
                .iter()
                .map(|(_, out)| out.clone())
                .collect::<Vec<_>>(),
            [
                vec!["0  TRUE"],
                vec!["1  TRUE"],
                vec!["1  TRUE"],
                vec!["2  TRUE"]
            ]
        );
        let nested = fixture(
            "{B \\in {A \\in SUBSET (SUBSET (1..5)) : Print(10, TRUE)} : Print(20, TRUE)}",
            options,
        );
        let result = nested
            .ev
            .init_states(&nested.inits, &mut |_| Flow::Stop)
            .expect("first nested accepted value");
        assert_eq!(result.emitted, 1);
        assert_eq!(nested.ev.take_output(), ["10  TRUE", "20  TRUE"]);
    }
}

#[test]
fn captured_let_values_remain_distinct_across_original_scopes() {
    for options in [Options::dive(), Options::tlc()] {
        let f = fixture(
            "LET k == 1 IN {A \\in SUBSET (SUBSET (1..2)) : \\A e \\in A : Cardinality(e) = k}",
            options,
        );
        let mut values = Vec::new();
        let result =
            f.ev.init_states(&f.inits, &mut |s| {
                values.push(s.state[0].clone().expect("x"));
                Flow::Continue
            })
            .expect("captured constant");
        assert_eq!(result.emitted, 4);
        assert_eq!(values[0], Value::empty_set(options.ctx));
        let dependent = fixture(
            "{A \\in SUBSET (SUBSET (1..2)) : Cardinality(A) <= 1}",
            options,
        );
        let result = dependent
            .ev
            .init_states(&dependent.inits, &mut |_| Flow::Continue)
            .expect("outer-dependent predicate is not pruned");
        assert_eq!(result.emitted, 5);
    }
}

#[test]
fn actual_partial_state_is_captured_separately_for_each_filter_creation() {
    let source = "---- MODULE Cursor ----\nEXTENDS Integers, FiniteSets\nVARIABLES x,v\nReference == {A \\in SUBSET (SUBSET (1..2)) : \\A e \\in A : Cardinality(e) = v}\nInit == v \\in {0,1} /\\ x \\in Reference /\\ Cardinality(x) <= 1\nNext == x' = x /\\ v' = v\n====\n";
    for options in [Options::dive(), Options::tlc()] {
        let f = from_source(source, options);
        let mut values = Vec::new();
        let result =
            f.ev.init_states(&f.inits, &mut |state| {
                values.push((
                    state.state[0].clone().expect("x"),
                    state.state[1].clone().expect("v"),
                ));
                Flow::Continue
            })
            .expect("actual state-dependent predicate, without pruning");
        let empty = Value::empty_set(options.ctx);
        let singleton = |n| {
            Value::set(
                vec![Value::set(vec![Value::Int(n)], options.ctx).expect("element")],
                options.ctx,
            )
            .expect("singleton family")
        };
        let empty_member = Value::set(vec![empty.clone()], options.ctx).expect("empty member");
        assert_eq!(
            values,
            [
                (empty.clone(), 0.into()),
                (empty_member, 0.into()),
                (empty, 1.into()),
                (singleton(1), 1.into()),
                (singleton(2), 1.into())
            ]
        );
        assert_eq!(result.emitted, 5);
    }
}

#[derive(Debug)]
struct StopAfterOne(AtomicUsize);
impl HostControl for StopAfterOne {
    fn check(&self) -> Result<(), Interrupted> {
        if self.0.fetch_add(1, Ordering::Relaxed) == 0 {
            Ok(())
        } else {
            Err(Interrupted::new(459))
        }
    }
}

#[test]
fn all_rejected_candidates_keep_typed_control_and_do_not_cache_the_stop() {
    for options in [Options::dive(), Options::tlc()] {
        let f = fixture(
            "{A \\in SUBSET (SUBSET (1..5)) : Print(99, FALSE)}",
            options,
        );
        let control = Arc::new(StopAfterOne(AtomicUsize::new(0)));
        let scope =
            f.ev.host_control(Some(control.clone()))
                .expect("control scope");
        let error =
            f.ev.init_states(&f.inits, &mut |_| Flow::Continue)
                .expect_err("bounded original predicate work");
        assert_eq!(error.error.class(), ErrorClass::Interrupted);
        assert_eq!(
            error.error.interruption().expect("typed receipt").token(),
            459
        );
        assert_eq!(control.0.load(Ordering::Relaxed), 2);
        assert_eq!(error.emitted, 0);
        let output = f.ev.take_output();
        assert_ne!(output.len(), 0);
        assert!(output.iter().all(|line| line == "99  FALSE"));
        drop(scope);
        let witness = f.ev.ir().root_def("Witness").expect("Witness");
        assert!(
            f.ev.enabled_scoped(
                &f.ev.scope(witness).expect("scope"),
                f.ev.ir().def(witness).body,
                None,
                Arc::from([Some(0.into())])
            )
            .expect("fresh unfiltered witness")
        );
    }
}

#[derive(Debug)]
struct PanicOnPrint(Arc<AtomicUsize>);
impl dive_eval::Host for PanicOnPrint {
    fn check_native_access(&self, op: dive_ir::StdOp) -> Result<(), dive_eval::HostError> {
        if op == dive_ir::StdOp::TlcPrintT {
            self.0.fetch_add(1, Ordering::Relaxed);
            std::panic::resume_unwind(Box::new("original host callback panic"));
        }
        Ok(())
    }
}

#[test]
fn a_real_host_panic_unwinds_the_cursor_without_poisoning_later_evaluation() {
    for options in [Options::dive(), Options::tlc()] {
        let f = fixture("{A \\in SUBSET (SUBSET (1..5)) : PrintT(100)}", options);
        let hits = Arc::new(AtomicUsize::new(0));
        f.ev.set_host(Arc::new(PanicOnPrint(hits.clone())));
        let mut emitted = 0;
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            f.ev.init_states(&f.inits, &mut |_| {
                emitted += 1;
                Flow::Continue
            })
        }))
        .expect_err("actual host unwind");
        assert_eq!(
            panic.downcast_ref::<&str>(),
            Some(&"original host callback panic")
        );
        assert_eq!(hits.load(Ordering::Relaxed), 1);
        assert_eq!(emitted, 0);
        assert_eq!(f.ev.take_output(), Vec::<String>::new());
        drop(f.ev.replace_host(None));
        let result =
            f.ev.init_states(&f.inits, &mut |_| Flow::Stop)
                .expect("fresh original predicate after unwind");
        assert_eq!(result.emitted, 1);
        assert!(result.stopped);
        assert_eq!(f.ev.take_output(), ["100"]);
    }
}

#[derive(Debug)]
struct OrdinaryHost;
impl dive_eval::Host for OrdinaryHost {}

#[test]
fn operative_singleton_projection_preserves_real_init_successor_sinks_and_partial_state() {
    let text = "---- MODULE Cursor ----\nEXTENDS Integers,FiniteSets,TLC\nVARIABLES x,mom,dist\nReference == {A \\in SUBSET (SUBSET {10,20,30}) : \\A e \\in A : Cardinality(e)=2}\nInit == /\\ mom=[n \\in {10,20,30} |-> n] /\\ dist=[n \\in {10,20,30} |-> 0] /\\ x \\in Reference\nNext == /\\ mom'=mom /\\ dist'=dist /\\ x' \\in Reference\n====\n";
    for options in [Options::dive(), Options::tlc()] {
        let mut runs = Vec::new();
        for operative in [false, true] {
            let f = from_source(text, options);
            f.ev.set_host(Arc::new(OrdinaryHost));
            let _scope = operative.then(|| {
                f.ev.native_hook_scope(dive_eval::NativeHooks::Unrestricted)
                    .expect("domain0 operative binding")
            });
            let mut initial = Vec::new();
            let init =
                f.ev.init_states(&f.inits, &mut |successor| {
                    assert_eq!(successor.ordinal, initial.len() as u64);
                    initial.push(successor.state.to_vec());
                    Flow::Continue
                })
                .expect("full original Init");
            assert_eq!(init.emitted, 8);
            assert!(!init.stopped);
            assert_eq!(initial[0][0], Some(Value::empty_set(options.ctx)));
            let mut successors = Vec::new();
            let next =
                f.ev.successors(&f.actions, &initial[0].clone().into(), &mut |successor| {
                    assert_eq!(successor.ordinal, successors.len() as u64);
                    successors.push(successor.state.to_vec());
                    Flow::Continue
                })
                .expect("full original Next");
            assert_eq!(next.emitted, 8);
            assert!(!next.stopped);
            // An actual sink stop on empty precedes inspection/learning.
            let stop =
                f.ev.init_states(&f.inits, &mut |successor| {
                    assert_eq!(successor.state[0], Some(Value::empty_set(options.ctx)));
                    Flow::Stop
                })
                .expect("early original sink stop");
            assert_eq!(stop.emitted, 1);
            assert!(stop.stopped);
            runs.push((initial, successors, f.ev.take_output()));
        }
        assert_eq!(
            runs[1], runs[0],
            "reference executes first, all states/ordinals retained"
        );
    }
}
