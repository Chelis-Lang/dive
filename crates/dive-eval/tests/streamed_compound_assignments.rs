//! Compound assignments reach continuations without retaining every output.
#![allow(
    clippy::expect_used,
    reason = "bounded fixture construction and assertions"
)]

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use dive_eval::next::{Action, Flow};
use dive_eval::{ErrorClass, Evaluator, HostControl, Interrupted, Model, Options};
use dive_ir::actions::{ActionName, split};
use dive_value::{Ctx, Interner, Value};

fn bind(domain: &str, guard: &str, options: Options) -> (Evaluator, Vec<Action>, Vec<Action>) {
    let source = format!(
        "---- MODULE Compound ----\nEXTENDS Integers, TLC\nVARIABLE x\nInit == x \\in {domain} /\\ ({guard})\nNext == x' \\in {domain} /\\ ({guard})\nCompoundCandidateWitness == x' \\in {domain}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("Compound.tla", &source, &dive_sema::Options::default());
    assert!(analysis.accepted(), "{:?}", analysis.diagnostics());
    let ir = Arc::new(dive_ir::lower(&analysis).expect("IR"));
    let init = ir.root_def("Init").expect("Init");
    let next = ir.root_def("Next").expect("Next");
    let evaluator = Evaluator::new(Model::new(ir.clone()), options).expect("evaluator");
    let inits = vec![
        evaluator
            .init_predicate(init, 0)
            .expect("initial predicate"),
    ];
    let tree = split(
        &ir,
        ir.def(next).body,
        ActionName::Def(next),
        next,
        &std::collections::BTreeMap::new(),
    );
    let actions = evaluator.actions(&tree, next, 1).expect("actions");
    (evaluator, inits, actions)
}

fn small_cases(ctx: Ctx) -> Vec<(&'static str, Vec<Value>)> {
    let names = Interner::new();
    let a = names.intern("a");
    let b = names.intern("b");
    let subset = |values: Vec<Value>| Value::set(values, ctx).expect("explicit set");
    vec![
        (
            "SUBSET {2, 1}",
            vec![
                subset(vec![]),
                subset(vec![Value::Int(1)]),
                subset(vec![Value::Int(2)]),
                subset(vec![Value::Int(1), Value::Int(2)]),
            ],
        ),
        (
            "{2, 1} \\X {4, 3}",
            [(1, 3), (1, 4), (2, 3), (2, 4)]
                .into_iter()
                .map(|(a, b)| Value::tuple(vec![Value::Int(a), Value::Int(b)]))
                .collect(),
        ),
        (
            "[1..2 -> {4, 3}]",
            [(3, 3), (3, 4), (4, 3), (4, 4)]
                .into_iter()
                .map(|(a, b)| Value::tuple_fcn(vec![Value::Int(a), Value::Int(b)]))
                .collect(),
        ),
        (
            "[a: 1..2, b: 3..4]",
            [(1, 3), (1, 4), (2, 3), (2, 4)]
                .into_iter()
                .map(|(x, y)| {
                    Value::record(
                        vec![(a.clone(), Value::Int(x)), (b.clone(), Value::Int(y))],
                        ctx,
                    )
                    .expect("record")
                })
                .collect(),
        ),
        (r"{1, TRUE} \X {}", vec![]),
        ("[a: {1, TRUE}, b: {}]", vec![]),
        ("SUBSET {}", vec![subset(vec![])]),
        ("(1..0) \\X (1..2)", vec![]),
        ("[{} -> {}]", vec![Value::tuple_fcn(vec![])]),
        ("[1..2 -> {}]", vec![]),
    ]
}

#[test]
fn complete_small_enumerations_keep_values_order_ordinals_and_empty_cases() {
    for options in [Options::dive(), Options::tlc()] {
        for (domain, expected) in small_cases(options.ctx) {
            let (ev, inits, actions) = bind(domain, "TRUE", options);
            for initial in [true, false] {
                let mut actual = Vec::new();
                let mut sink = |successor: &dive_eval::next::Successor<'_>| {
                    assert_eq!(successor.ordinal, actual.len() as u64);
                    actual.push(successor.state[0].clone().expect("assigned"));
                    Flow::Continue
                };
                let outcome = if initial {
                    ev.init_states(&inits, &mut sink)
                } else {
                    ev.successors(&actions, &Arc::from([Some(Value::Int(0))]), &mut sink)
                }
                .expect("complete enumeration");
                assert_eq!(actual, expected, "{domain}, initial={initial}");
                assert_eq!(outcome.emitted, expected.len() as u64);
                assert!(!outcome.stopped);
            }
        }
    }
}

const WIDE: [&str; 4] = [
    "SUBSET (1..31)",
    "(1..50000) \\X (1..50000)",
    "[1..2 -> 1..50000]",
    "[a: 1..50000, b: 1..50000]",
];

#[test]
fn wide_domains_stop_at_the_first_initial_or_successor_and_enabled_witness() {
    for options in [Options::dive(), Options::tlc()] {
        for domain in WIDE {
            let (ev, inits, actions) = bind(domain, "TRUE", options);
            for initial in [true, false] {
                let mut actual = Vec::new();
                let mut sink = |successor: &dive_eval::next::Successor<'_>| {
                    assert_eq!(successor.ordinal, 0);
                    actual.push(successor.state[0].clone().expect("assigned"));
                    Flow::Stop
                };
                let outcome = if initial {
                    ev.init_states(&inits, &mut sink)
                } else {
                    ev.successors(&actions, &Arc::from([Some(Value::Int(0))]), &mut sink)
                }
                .expect("first output without complete compound allocation");
                assert_eq!(actual.len(), 1);
                assert_eq!(outcome.emitted, 1);
                assert!(outcome.stopped);
            }
            let any = ev
                .ir()
                .root_def("CompoundCandidateWitness")
                .expect("CompoundCandidateWitness");
            assert!(
                ev.enabled_scoped(
                    &ev.scope(any).expect("scope"),
                    ev.ir().def(any).body,
                    None,
                    Arc::from([Some(Value::Int(0))])
                )
                .expect("first witness")
            );
        }
    }
}

#[derive(Debug)]
struct StopAfterOne(AtomicUsize);
impl HostControl for StopAfterOne {
    fn check(&self) -> Result<(), Interrupted> {
        if self.0.fetch_add(1, Ordering::Relaxed) == 0 {
            Ok(())
        } else {
            Err(Interrupted::new(73))
        }
    }
}

#[test]
fn rejected_compound_candidates_reach_typed_control_without_emitting_states() {
    for options in [Options::dive(), Options::tlc()] {
        for domain in WIDE {
            let (ev, inits, actions) = bind(domain, "Print(99, FALSE)", options);
            for route in 0..3 {
                let control = Arc::new(StopAfterOne(AtomicUsize::new(0)));
                let scope = ev.host_control(Some(control.clone())).expect("control");
                let mut emitted = 0;
                let mut sink = |_: &dive_eval::next::Successor<'_>| {
                    emitted += 1;
                    Flow::Continue
                };
                let error = match route {
                    0 => {
                        ev.init_states(&inits, &mut sink)
                            .expect_err("initial candidate stop")
                            .error
                    }
                    1 => {
                        ev.successors(&actions, &Arc::from([Some(Value::Int(0))]), &mut sink)
                            .expect_err("successor candidate stop")
                            .error
                    }
                    _ => {
                        let next = ev.ir().root_def("Next").expect("Next");
                        ev.enabled_scoped(
                            &ev.scope(next).expect("scope"),
                            ev.ir().def(next).body,
                            None,
                            Arc::from([Some(Value::Int(0))]),
                        )
                        .expect_err("enabled candidate stop")
                    }
                };
                assert_eq!(error.class(), ErrorClass::Interrupted);
                assert_eq!(error.interruption().expect("receipt").token(), 73);
                assert_eq!(control.0.load(Ordering::Relaxed), 2);
                assert_eq!(emitted, 0);
                let output = ev.take_output();
                assert!(!output.is_empty(), "continuations ran before interruption");
                assert!(output.iter().all(|line| line == "99  FALSE"));
                drop(scope);
                let any = ev
                    .ir()
                    .root_def("CompoundCandidateWitness")
                    .expect("CompoundCandidateWitness");
                assert!(
                    ev.enabled_scoped(
                        &ev.scope(any).expect("scope"),
                        ev.ir().def(any).body,
                        None,
                        Arc::from([Some(Value::Int(0))])
                    )
                    .expect("retry has no cached interruption")
                );
            }
        }
    }
}
