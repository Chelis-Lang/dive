//! Candidate work remains interruptible when no complete successor is emitted.
#![allow(
    clippy::expect_used,
    reason = "finite control construction and assertions"
)]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use dive_eval::next::{Action, Flow};
use dive_eval::{
    ActionComposition, ErrorClass, Evaluator, HostControl, Interrupted, Model, Options, St,
};
use dive_ir::actions::{ActionName, split};
use dive_value::Value;

#[derive(Debug)]
struct Control {
    calls: AtomicUsize,
    allowed: usize,
}

impl Control {
    fn after(allowed: usize) -> Arc<Self> {
        Arc::new(Self {
            calls: AtomicUsize::new(0),
            allowed,
        })
    }
}

impl HostControl for Control {
    fn check(&self) -> Result<(), Interrupted> {
        if self.calls.fetch_add(1, Ordering::Relaxed) < self.allowed {
            Ok(())
        } else {
            Err(Interrupted::new(43))
        }
    }
}

fn bind(next: &str, options: Options) -> (Evaluator, Vec<Action>) {
    let source = format!(
        "---- MODULE Candidate ----\nEXTENDS Integers\nVARIABLE x\nInit == x=0\nNext == {next}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("Candidate.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let next = ir.root_def("Next").expect("Next");
    let evaluator = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
    let tree = split(
        &ir,
        ir.def(next).body,
        ActionName::Def(next),
        next,
        &std::collections::BTreeMap::new(),
    );
    let actions = evaluator.actions(&tree, next, 1).expect("actions");
    (evaluator, actions)
}

#[test]
fn finite_rejected_interval_candidates_interrupt_without_a_successor() {
    for options in [Options::dive(), Options::tlc()] {
        let (evaluator, actions) = bind("x' \\in 1..4096 /\\ FALSE", options);
        let control = Control::after(0);
        let scope = evaluator
            .host_control(Some(control.clone()))
            .expect("owner");
        let mut emitted = 0;
        let error = evaluator
            .successors(&actions, &Arc::from([Some(Value::Int(0))]), &mut |_| {
                emitted += 1;
                Flow::Continue
            })
            .expect_err("candidate search must reach the control without an emission");
        assert_eq!(error.error.class(), ErrorClass::Interrupted);
        assert_eq!(error.error.interruption().expect("receipt").token(), 43);
        assert!(control.calls.load(Ordering::Relaxed) > 0);
        assert_eq!((emitted, error.emitted, error.intermediate), (0, 0, 0));
        drop(scope);
        let retry = evaluator
            .successors(&actions, &Arc::from([Some(Value::Int(0))]), &mut |_| {
                Flow::Continue
            })
            .expect("retry without interrupted bindings or control");
        assert_eq!((retry.emitted, retry.intermediate), (0, 0));
        assert!(!retry.stopped);
    }
}

#[test]
fn rejected_materialized_and_big_integer_candidates_retain_typed_control() {
    for (domain, options) in [
        ("(1..4096) \\cup {0}", Options::tlc()),
        ("(1..4096) \\cup {0}", Options::dive()),
        ("9223372036854775808..9223372036854779903", Options::dive()),
    ] {
        let (evaluator, actions) = bind(&format!("x' \\in {domain} /\\ FALSE"), options);
        let control = Control::after(0);
        let _scope = evaluator
            .host_control(Some(control.clone()))
            .expect("owner");
        let error = evaluator
            .successors(&actions, &Arc::from([Some(Value::Int(0))]), &mut |_| {
                Flow::Continue
            })
            .expect_err("candidate interruption");
        assert_eq!(error.error.class(), ErrorClass::Interrupted);
        assert_eq!(error.error.interruption().expect("receipt").token(), 43);
        assert_eq!((error.emitted, error.intermediate), (0, 0));
        assert!(control.calls.load(Ordering::Relaxed) > 0);
    }
}

fn composed_options(mode: ActionComposition, compat: bool) -> Options {
    Options {
        action_composition: mode,
        ..if compat {
            Options::tlc()
        } else {
            Options::dive()
        }
    }
}

#[test]
fn composition_left_collection_interrupts_before_the_external_sink() {
    for compat in [false, true] {
        for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
            let (evaluator, actions) = bind(
                "(x' \\in 1..4096) \\cdot (x'=x)",
                composed_options(mode, compat),
            );
            let control = Control::after(0);
            let scope = evaluator
                .host_control(Some(control.clone()))
                .expect("owner");
            let mut emitted = 0;
            let error = evaluator
                .successors(&actions, &Arc::from([Some(Value::Int(0))]), &mut |_| {
                    emitted += 1;
                    Flow::Continue
                })
                .expect_err("interrupt private collection");
            assert_eq!(error.error.class(), ErrorClass::Interrupted);
            assert_eq!(error.error.interruption().expect("receipt").token(), 43);
            assert_eq!((emitted, error.emitted, error.intermediate), (0, 0, 0));
            drop(scope);
            let mut expected = 1;
            let retry = evaluator
                .successors(
                    &actions,
                    &Arc::from([Some(Value::Int(0))]),
                    &mut |successor| {
                        assert_eq!(successor.state[0], Some(Value::Int(expected)));
                        expected += 1;
                        Flow::Continue
                    },
                )
                .expect("retry complete composition");
            assert_eq!(retry.emitted, 4096);
            assert_eq!(
                retry.intermediate,
                if mode == ActionComposition::Tlc {
                    4096
                } else {
                    0
                }
            );
            assert!(!retry.stopped);
        }
    }
}

#[test]
fn each_composition_right_loop_interrupts_after_complete_left_collection() {
    for compat in [false, true] {
        for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
            for route in 0..3 {
                let (evaluator, actions) = bind(
                    "(x' \\in 1..4096) \\cdot FALSE",
                    composed_options(mode, compat),
                );
                // Exactly 4096 candidate checkpoints consume four due calls,
                // independent of the existing thread-local countdown position.
                // The fifth due call requires real RIGHT-side loop progress.
                let control = Control::after(4);
                let scope = evaluator
                    .host_control(Some(control.clone()))
                    .expect("owner");
                let next = evaluator.ir().root_def("Next").expect("Next");
                let state = Arc::from([Some(Value::Int(0))]);
                let error = match route {
                    0 => {
                        let error = evaluator
                            .successors(&actions, &state, &mut |_| Flow::Continue)
                            .expect_err("RIGHT next-state search interruption");
                        assert_eq!(error.emitted, 0);
                        assert_eq!(
                            error.intermediate,
                            if mode == ActionComposition::Tlc {
                                4096
                            } else {
                                0
                            }
                        );
                        error.error
                    }
                    1 => evaluator
                        .eval_def(next, &St::action(state.clone(), state.clone()))
                        .expect_err("RIGHT action predicate interruption"),
                    _ => evaluator
                        .enabled_scoped(
                            &evaluator.scope(next).expect("scope"),
                            evaluator.ir().def(next).body,
                            None,
                            state.clone(),
                        )
                        .expect_err("RIGHT ENABLED search interruption"),
                };
                assert_eq!(error.class(), ErrorClass::Interrupted);
                assert_eq!(error.interruption().expect("receipt").token(), 43);
                assert_eq!(control.calls.load(Ordering::Relaxed), 5);
                drop(scope);
                assert_eq!(
                    evaluator
                        .eval_def(next, &St::action(state.clone(), state.clone()))
                        .expect("retry false predicate"),
                    Value::Bool(false)
                );
                assert!(
                    !evaluator
                        .enabled_scoped(
                            &evaluator.scope(next).expect("scope"),
                            evaluator.ir().def(next).body,
                            None,
                            state,
                        )
                        .expect("retry disabled composition")
                );
            }
        }
    }
}

#[test]
fn an_earlier_genuine_error_or_stopped_sink_runs_no_later_checkpoint() {
    // A new thread starts with a full amortization interval. An early deciding
    // event must return immediately, before any later candidate can trip it.
    std::thread::spawn(|| {
        for options in [Options::dive(), Options::tlc()] {
            let (evaluator, actions) = bind("x' \\in 1..4096 /\\ (1 \\div 0)=0", options);
            let control = Control::after(0);
            let _scope = evaluator
                .host_control(Some(control.clone()))
                .expect("owner");
            let error = evaluator
                .successors(&actions, &Arc::from([Some(Value::Int(0))]), &mut |_| {
                    Flow::Continue
                })
                .expect_err("earlier division error");
            assert_eq!(error.error.class(), ErrorClass::Eval);
            assert_eq!(control.calls.load(Ordering::Relaxed), 0);
        }
        for compat in [false, true] {
            for mode in [ActionComposition::Tla, ActionComposition::Tlc] {
                let (evaluator, actions) = bind(
                    "(x'=1) \\cdot (x' \\in 1..4096)",
                    composed_options(mode, compat),
                );
                let control = Control::after(0);
                let _scope = evaluator
                    .host_control(Some(control.clone()))
                    .expect("owner");
                let outcome = evaluator
                    .successors(
                        &actions,
                        &Arc::from([Some(Value::Int(0))]),
                        &mut |successor| {
                            assert_eq!(successor.state[0], Some(Value::Int(1)));
                            assert_eq!(successor.ordinal, 0);
                            Flow::Stop
                        },
                    )
                    .expect("already selected sink event");
                assert!(outcome.stopped);
                assert_eq!(outcome.emitted, 1);
                assert_eq!(
                    outcome.intermediate,
                    u64::from(mode == ActionComposition::Tlc)
                );
                assert_eq!(control.calls.load(Ordering::Relaxed), 0);
            }
        }
    })
    .join()
    .expect("priority control thread");
}
