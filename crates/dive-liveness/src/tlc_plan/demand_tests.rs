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
    assert!(
        normalized
            .into_source_program()
            .expect("no row program")
            .is_none()
    );
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
