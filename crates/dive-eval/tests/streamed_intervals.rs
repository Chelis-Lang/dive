//! Interval enumeration preserves values/order and honors a sink stop.
#![allow(clippy::expect_used, reason = "test construction and assertions")]

use std::sync::Arc;

use dive_eval::next::{Action, Flow};
use dive_eval::{Evaluator, Model, Options};
use dive_ir::actions::{ActionName, split};
use dive_value::Value;

type Bound = (Evaluator, Vec<Action>, Vec<Action>);

fn bind(domain: &str, options: Options) -> Bound {
    let source = format!(
        "---- MODULE Interval ----\nEXTENDS Integers\nVARIABLE x\nInit == x \\in {domain}\nNext == x' \\in {domain}\n====\n"
    );
    let analysis =
        dive_sema::analyze_source("Interval.tla", &source, &dive_sema::Options::default());
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

#[test]
fn initial_and_successor_intervals_keep_ascending_order_and_endpoints() {
    for options in [Options::default(), Options::tlc()] {
        for (domain, expected) in [
            ("-2..2", vec![-2, -1, 0, 1, 2]),
            ("2..1", vec![]),
            ("0..0", vec![0]),
            (
                "2147483645..2147483647",
                vec![2_147_483_645, 2_147_483_646, 2_147_483_647],
            ),
            (
                "(-2147483647-1)..(-2147483647+1)",
                vec![-2_147_483_648, -2_147_483_647, -2_147_483_646],
            ),
        ] {
            let (evaluator, inits, actions) = bind(domain, options);
            let mut initial = Vec::new();
            let outcome = evaluator
                .init_states(&inits, &mut |successor| {
                    assert_eq!(successor.ordinal, initial.len() as u64);
                    initial.push(successor.state[0].clone().expect("assigned"));
                    Flow::Continue
                })
                .expect("initial enumeration");
            let expected: Vec<Value> = expected.into_iter().map(Value::Int).collect();
            assert_eq!(initial, expected);
            assert!(!outcome.stopped);
            let mut next = Vec::new();
            let outcome = evaluator
                .successors(
                    &actions,
                    &Arc::from([Some(Value::Int(0))]),
                    &mut |successor| {
                        assert_eq!(successor.ordinal, next.len() as u64);
                        next.push(successor.state[0].clone().expect("assigned"));
                        Flow::Continue
                    },
                )
                .expect("successor enumeration");
            assert_eq!(next, expected);
            assert!(!outcome.stopped);
        }
    }
}

#[test]
fn sink_stops_preserve_ordinals_and_big_integer_boundary() {
    for (domain, expected, options) in [
        (
            "0..100000",
            vec![Value::Int(0), Value::Int(1), Value::Int(2)],
            Options::tlc(),
        ),
        (
            "9223372036854775806..9223372036854775810",
            vec![
                Value::Int(i64::MAX - 1),
                Value::Int(i64::MAX),
                Value::int(
                    dive_value::Int::from_literal("9223372036854775808", dive_value::IntMode::Big)
                        .expect("big integer"),
                ),
            ],
            Options::default(),
        ),
    ] {
        let (evaluator, inits, actions) = bind(domain, options);
        for initial in [true, false] {
            let mut values = Vec::new();
            let mut sink = |successor: &dive_eval::next::Successor<'_>| {
                assert_eq!(successor.ordinal, values.len() as u64);
                values.push(successor.state[0].clone().expect("assigned"));
                if values.len() == 3 {
                    Flow::Stop
                } else {
                    Flow::Continue
                }
            };
            let outcome = if initial {
                evaluator.init_states(&inits, &mut sink)
            } else {
                evaluator.successors(&actions, &Arc::from([Some(Value::Int(0))]), &mut sink)
            }
            .expect("stopped enumeration");
            assert_eq!(values, expected);
            assert_eq!(outcome.emitted, 3);
            assert!(outcome.stopped);
        }
    }
}
