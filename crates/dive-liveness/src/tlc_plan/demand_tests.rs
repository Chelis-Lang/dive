//! Demand normalization is tested independently of evaluator execution.
#![allow(clippy::expect_used, reason = "checked formula controls")]

use super::*;
use crate::formula::{Atom, AtomKind};

fn state(index: usize) -> Node {
    Node::Atom(Atom {
        kind: AtomKind::State,
        index,
    })
}

fn violation(mut nodes: Vec<Node>, root: usize) -> Formula {
    let negated = nodes.len();
    nodes.push(Node::Not(root));
    nodes.push(Node::And(vec![negated]));
    let last = nodes.len() - 1;
    Formula::new(nodes, last).expect("checked lower_violation envelope")
}

#[test]
fn current_state_action_bin_keeps_priority_over_the_first_source_state_bin() {
    // P == (<>[]TypeBad) \/ ([]<>DivBad).
    let formula = violation(
        vec![
            state(0),
            Node::Always(0),
            Node::Eventually(1),
            state(1),
            Node::Eventually(3),
            Node::Always(4),
            Node::Or(vec![2, 5]),
        ],
        6,
    );
    let program = compile(&[(&formula, 2)], 0, Limits::default())
        .expect("compile")
        .expect("complete bins");
    let mut reached = Vec::new();
    let result = program.evaluate(|atom| {
        reached.push(atom);
        if atom == 1 {
            Err("division")
        } else {
            Err("type")
        }
    });
    assert_eq!(result, Err("division"));
    assert_eq!(reached, vec![1]);
    assert!(
        program.certificate().is_none(),
        "MUST alone grants no runtime authority"
    );
}

#[test]
fn an_unknown_short_circuit_right_operand_cannot_be_missing_from_backend_truth() {
    let formula = violation(
        vec![
            state(0),
            state(1),
            Node::And(vec![0, 1]),
            Node::Eventually(2),
            Node::Always(3),
        ],
        4,
    );
    assert!(
        compile(&[(&formula, 2)], 0, Limits::default())
            .expect("compile")
            .is_none()
    );
}

#[test]
fn always_executed_other_bins_can_complete_must_without_changing_short_circuits() {
    // One copied original occurrence is reached through two normalized bins.
    let formula = violation(
        vec![
            state(0),
            state(1),
            Node::And(vec![0, 1]),
            Node::Eventually(2),
            Node::Always(3),
            Node::Always(1),
            Node::Eventually(5),
            Node::And(vec![4, 6]),
        ],
        7,
    );
    let program = compile(&[(&formula, 2)], 0, Limits::default())
        .expect("compile")
        .expect("union covers both");
    let mut reached = Vec::new();
    let result: Result<(), ()> = program.evaluate(|atom| {
        reached.push(atom);
        Ok(atom != 0)
    });
    assert_eq!(result, Ok(()));
    assert_eq!(
        reached,
        vec![0, 1],
        "right operand skipped in first bin, read by next bin"
    );
}

#[test]
fn equal_local_atom_numbers_in_different_properties_remain_different_occurrences() {
    let first = violation(vec![state(0), Node::Eventually(0), Node::Always(1)], 2);
    let second = first.clone();
    let program = compile(&[(&first, 1), (&second, 1)], 0, Limits::default())
        .expect("compile")
        .expect("two independent slots");
    assert_eq!(
        program.slots,
        vec![
            Slot {
                property: 0,
                atom: 0
            },
            Slot {
                property: 1,
                atom: 0
            }
        ]
    );
    let mut reached = Vec::new();
    let result: Result<(), ()> = program.evaluate(|atom| {
        reached.push(atom);
        Ok(true)
    });
    assert_eq!(result, Ok(()));
    assert_eq!(reached, vec![0, 1]);
}

#[test]
fn residual_and_zero_solution_programs_keep_the_old_route() {
    let residual = violation(vec![state(0), Node::Always(0)], 1);
    let zero = violation(vec![Node::Bool(true)], 0);
    assert!(
        compile(&[(&residual, 1)], 0, Limits::default())
            .expect("residual")
            .is_none()
    );
    assert!(
        compile(&[(&zero, 0)], 0, Limits::default())
            .expect("zero")
            .is_none()
    );
}

#[test]
fn startup_zero_is_a_distinct_complete_normalization_not_a_row_certificate() {
    // Original bad state atom survives conversion; only the combined temporal
    // implication's FALSE negation discards it. No predicate is invoked here.
    let tautology = violation(
        vec![
            state(0),
            Node::Eventually(0),
            Node::Always(1),
            Node::Not(2),
            Node::Bool(true),
            Node::Or(vec![3, 4]),
        ],
        5,
    );
    let normalized = normalize(&[(&tautology, 1)], 0, Limits::default())
        .expect("complete conversion")
        .expect("envelope");
    assert!(normalized.zero_solutions());
    assert!(normalized.into_program().expect("no row program").is_none());
}

#[test]
fn mixed_claim_orders_and_semantic_tautologies_do_not_invent_zero_solutions() {
    let tautology = violation(vec![Node::Bool(true)], 0);
    let claim = violation(vec![state(0), Node::Eventually(0), Node::Always(1)], 2);
    for formulas in [
        vec![(&tautology, 0), (&claim, 1)],
        vec![(&claim, 1), (&tautology, 0)],
    ] {
        let normalized = normalize(&formulas, 0, Limits::default())
            .expect("normalization")
            .expect("envelope");
        assert!(!normalized.zero_solutions());
    }
    for nodes in [
        vec![Node::Bool(true), Node::Eventually(0)],
        vec![Node::Bool(true), Node::Always(0), Node::Eventually(1)],
    ] {
        let root = nodes.len() - 1;
        let formula = violation(nodes, root);
        let normalized = normalize(&[(&formula, 0)], 0, Limits::default())
            .expect("normalization")
            .expect("envelope");
        assert!(!normalized.zero_solutions());
    }
    let limited = Limits {
        expansion_steps: 0,
        ..Limits::default()
    };
    assert!(matches!(
        normalize(&[(&tautology, 0)], 0, limited),
        Err(FormulaError::Limit(_))
    ));
}
