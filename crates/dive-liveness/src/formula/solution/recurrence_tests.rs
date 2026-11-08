//! Recurrence and persistence split from temporal memory, versus direct
//! periodic words and bounded on-the-fly products.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use super::{CycleConditions, decompose};
use crate::Graph;
use crate::formula::{
    Atom, AtomKind, Formula, FormulaError, Limits as FormulaLimits, Node, compile_residual,
};
use crate::product::{AcceptancePolicy, ImplicitProduct, Labels, Limits, ProductError};

fn none() -> CycleConditions {
    CycleConditions {
        always: vec![],
        recurring: vec![],
    }
}

// Whether any alternative of any solution group of `formula` accepts a lasso
// of `graph`: decomposed, compiled and searched as the temporal route does,
// under the legacy policy or the one `accepts_with` names.
fn accepts(
    formula: &Formula,
    graph: &Graph,
    labels: &Labels,
    limits: Limits,
) -> Result<bool, ProductError> {
    accepts_with(formula, graph, labels, limits, AcceptancePolicy::Legacy)
}

fn accepts_with(
    formula: &Formula,
    graph: &Graph,
    labels: &Labels,
    limits: Limits,
    policy: AcceptancePolicy,
) -> Result<bool, ProductError> {
    let groups = decompose(formula, FormulaLimits::default()).expect("decompose");
    let mut found = false;
    for group in &groups {
        let tableau =
            compile_residual(&group.core, FormulaLimits::default()).expect("residual tableau");
        let product = ImplicitProduct::new(graph, &tableau, labels, limits, policy)?;
        for conditions in &group.alternatives {
            found |= product
                .find_lasso_with_conditions(labels, conditions)?
                .is_some();
        }
    }
    Ok(found)
}

fn cases() -> Vec<Formula> {
    let base = vec![
        Node::Atom(Atom {
            kind: AtomKind::State,
            index: 0,
        }),
        Node::Atom(Atom {
            kind: AtomKind::Action,
            index: 0,
        }),
        Node::Not(0),
        Node::Eventually(0),
        Node::Always(3),
        Node::Eventually(1),
        Node::Always(5),
    ];
    let mut cases = vec![
        Formula::new(base.clone(), 4).expect("state recurrence"),
        Formula::new(base.clone(), 6).expect("action recurrence"),
    ];
    for root in [
        Node::And(vec![4, 6]),
        Node::Or(vec![4, 6]),
        Node::And(vec![2, 6]),
        Node::And(vec![4, 4, 6]),
        Node::Or(vec![4, 1]),
        Node::And(vec![]),
        Node::Or(vec![]),
    ] {
        let mut nodes = base.clone();
        nodes.push(root);
        let f = Formula::new(nodes.clone(), nodes.len() - 1).expect("formula");
        cases.push(f);
        nodes.push(Node::Not(nodes.len() - 1));
        cases.push(Formula::new(nodes.clone(), nodes.len() - 1).expect("negated formula"));
    }
    // Shared GF under another connective, nested temporal operands (retained),
    // and boolean state/action operands containing both conjunction and negation.
    for operand in [
        Node::And(vec![0, 1]),
        Node::Or(vec![0, 1]),
        Node::Not(1),
        Node::Bool(true),
        Node::Bool(false),
        Node::And(vec![]),
        Node::Or(vec![]),
        Node::Always(0),
    ] {
        let mut nodes = base.clone();
        let i = nodes.len();
        nodes.push(operand);
        nodes.push(Node::Eventually(i));
        nodes.push(Node::Always(i + 1));
        nodes.push(Node::And(vec![i + 2, 4]));
        cases.push(Formula::new(nodes.clone(), nodes.len() - 1).expect("mixed recurrence"));
    }
    cases
}

#[test]
fn every_decomposed_short_word_matches_original_temporal_truth() {
    for formula in cases() {
        for len in 1..=4 {
            for bits in 0..(1_u32 << (2 * len)) {
                let states: Vec<Vec<bool>> =
                    (0..len).map(|i| vec![bits & (1 << (2 * i)) != 0]).collect();
                let actions: Vec<Vec<bool>> =
                    (0..len).map(|i| vec![bits & (2 << (2 * i)) != 0]).collect();
                for back in 0..len {
                    let graph = Graph::new(
                        (0..len)
                            .map(|i| vec![if i + 1 == len { back } else { i + 1 }])
                            .collect(),
                        vec![0],
                    )
                    .expect("word graph");
                    let labels = Labels {
                        states: vec![states.iter().map(|v| v[0]).collect()],
                        actions: vec![actions.clone()],
                    };
                    let expected = formula
                        .evaluate_word(&states, &actions, back, 100_000)
                        .expect("direct word")[0];
                    for policy in [
                        AcceptancePolicy::Legacy,
                        AcceptancePolicy::compact_default(),
                    ] {
                        assert_eq!(
                            accepts_with(&formula, &graph, &labels, Limits::default(), policy)
                                .expect("search"),
                            expected,
                            "{formula:?} {policy:?} bits={bits} len={len} back={back}"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn closing_edge_fulfills_recurrence_but_prefix_alone_does_not() {
    let formula = Formula::new(
        vec![
            Node::Atom(Atom {
                kind: AtomKind::Action,
                index: 0,
            }),
            Node::Eventually(0),
            Node::Always(1),
        ],
        2,
    )
    .expect("recurrence");
    let groups = decompose(&formula, FormulaLimits::default()).expect("decompose");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].alternatives.len(), 1);
    let conditions = &groups[0].alternatives[0];
    assert_eq!(
        (conditions.always.len(), conditions.recurring.len()),
        (0, 1)
    );
    let tableau =
        compile_residual(&groups[0].core, FormulaLimits::default()).expect("residual tableau");
    let graph = Graph::new(vec![vec![1], vec![1]], vec![0]).expect("prefix and closing self-loop");
    let product = |labels: &Labels| {
        ImplicitProduct::new(
            &graph,
            &tableau,
            labels,
            Limits::default(),
            AcceptancePolicy::Legacy,
        )
        .expect("product")
        .find_lasso_with_conditions(labels, &none())
    };
    for (bits, expected) in [
        (vec![vec![false], vec![true]], true),
        (vec![vec![true], vec![false]], false),
    ] {
        let labels = Labels {
            states: vec![],
            actions: vec![bits],
        };
        assert_eq!(
            accepts(&formula, &graph, &labels, Limits::default()).expect("search"),
            expected
        );
        // Dropping recurrence is a planted bug: it incorrectly accepts the
        // second word, where the action occurs only in the finite prefix.
        assert!(product(&labels).expect("planted omission").is_some());
    }
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![false], vec![true]]],
    };
    assert!(matches!(
        accepts(
            &formula,
            &graph,
            &labels,
            Limits {
                acceptance_cells: 3,
                ..Limits::default()
            }
        ),
        Err(ProductError::Limit("acceptance cells"))
    ));
    let empty = Labels {
        states: vec![],
        actions: vec![],
    };
    assert!(matches!(
        ImplicitProduct::new(
            &graph,
            &tableau,
            &empty,
            Limits::default(),
            AcceptancePolicy::Legacy,
        )
        .expect("product")
        .find_lasso_with_conditions(&empty, conditions),
        Err(ProductError::Shape(_))
    ));
}

#[test]
fn shared_conjunctions_and_disjunctions_have_explicit_work_bounds() {
    for and in [true, false] {
        let mut nodes = vec![
            Node::Atom(Atom {
                kind: AtomKind::State,
                index: 0,
            }),
            Node::Eventually(0),
            Node::Always(1),
        ];
        for _ in 0..20 {
            let i = nodes.len() - 1;
            nodes.push(if and {
                Node::And(vec![i, i])
            } else {
                Node::Or(vec![i, i])
            });
        }
        let f = Formula::new(nodes.clone(), nodes.len() - 1).expect("small shared DAG");
        let result = decompose(&f, FormulaLimits::default());
        if and {
            // Unshared conjunct expansion is bounded, never silently truncated.
            assert!(matches!(result, Err(FormulaError::Limit(_))));
        } else {
            // Normalization shares the repeated disjunct: one `[]<>S` solution.
            let groups = result.expect("shared disjunction");
            assert_eq!(groups.len(), 1);
            assert_eq!(groups[0].alternatives.len(), 1);
            assert_eq!(groups[0].alternatives[0].recurring.len(), 1);
        }
    }
    let f = Formula::new(vec![Node::Bool(true)], 0).expect("constant");
    assert!(matches!(
        decompose(
            &f,
            FormulaLimits {
                formula_nodes: 0,
                ..FormulaLimits::default()
            }
        ),
        Err(FormulaError::Limit("formula nodes"))
    ));
}

#[test]
fn distinct_persistence_disjuncts_stop_at_the_decomposition_work_bound() {
    let mut nodes = Vec::new();
    let mut disjuncts = Vec::new();
    for index in 0..8 {
        let start = nodes.len();
        nodes.push(Node::Atom(Atom {
            kind: AtomKind::State,
            index,
        }));
        nodes.push(Node::Always(start));
        nodes.push(Node::Eventually(start + 1));
        disjuncts.push(start + 2);
    }
    nodes.push(Node::Or(disjuncts));
    let f = Formula::new(nodes.clone(), nodes.len() - 1).expect("eight FG disjuncts");
    // Unlike `[]<>`, persistence disjuncts never merge: one alternative each.
    let groups = decompose(&f, FormulaLimits::default()).expect("default work");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].alternatives.len(), 8);
    assert!(matches!(
        decompose(
            &f,
            FormulaLimits {
                expansion_steps: 16,
                ..FormulaLimits::default()
            }
        ),
        Err(FormulaError::Limit("solution decomposition work"))
    ));
}
