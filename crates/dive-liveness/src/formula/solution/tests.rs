//! Exhaustive short-word checks against the original temporal interpreter.
#![allow(clippy::expect_used, reason = "finite reference assertions")]
use super::*;
use crate::formula::{Atom, AtomKind};
use crate::product::{AcceptancePolicy, CompactProduct, Labels};
use crate::{Graph, product};

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
        Node::Bool(true),
        Node::Bool(false),
        Node::Not(0),
        Node::Not(1),
        Node::Eventually(0),
        Node::Always(6), // GF state
        Node::Always(0),
        Node::Eventually(8), // FG state
        Node::Eventually(1),
        Node::Always(10), // GF action
        Node::Always(1),
        Node::Eventually(12), // FG action
        Node::Eventually(3),
        Node::Always(14), // GF false
        Node::Always(3),
        Node::Eventually(16), // FG false
        Node::Or(vec![0, 1]),
        Node::Always(18),
        Node::Eventually(19), // FG(S or A)
        Node::Always(4),
        Node::Eventually(21), // FG not-state
        Node::And(vec![]),
        Node::Or(vec![]),
        Node::Eventually(24),
        Node::Always(25), // GF empty disjunction
        Node::Eventually(23),
        Node::Always(27), // GF empty conjunction
        Node::And(vec![0, 10]),
        Node::Eventually(29), // residual temporal memory
    ];
    let mut cases = Vec::new();
    for root in [0, 2, 3, 7, 9, 11, 13, 15, 17, 20, 22, 23, 24, 26, 28, 30] {
        cases.push(Formula::new(base.clone(), root).expect("seed"));
    }
    for left in [7, 9, 11, 13, 15, 20, 22, 26, 28, 30] {
        for right in [7, 9, 11, 13, 17, 20, 22, 26, 28, 30] {
            for and in [false, true] {
                let mut nodes = base.clone();
                nodes.push(if and {
                    Node::And(vec![left, right])
                } else {
                    Node::Or(vec![left, right])
                });
                let root = nodes.len() - 1;
                cases.push(Formula::new(nodes.clone(), root).expect("Boolean combination"));
                nodes.push(Node::Not(root));
                cases.push(Formula::new(nodes.clone(), root + 1).expect("negated combination"));
            }
        }
    }
    cases
}

#[test]
fn every_solution_union_matches_original_short_lassos() {
    for formula in cases() {
        let groups = decompose(&formula, Limits::default()).expect("decomposition");
        for len in 1..=3 {
            for bits in 0..1_u32 << (2 * len) {
                let states: Vec<_> = (0..len).map(|i| vec![bits & (1 << (2 * i)) != 0]).collect();
                let actions: Vec<_> = (0..len).map(|i| vec![bits & (2 << (2 * i)) != 0]).collect();
                for back in 0..len {
                    let graph = Graph::new(
                        (0..len)
                            .map(|i| vec![if i + 1 == len { back } else { i + 1 }])
                            .collect(),
                        vec![0],
                    )
                    .expect("word graph");
                    let labels = Labels {
                        states: vec![states.iter().map(|row| row[0]).collect()],
                        actions: vec![actions.clone()],
                    };
                    let expected = formula
                        .evaluate_word(&states, &actions, back, 100_000)
                        .expect("original truth")[0];
                    let mut found = false;
                    for group in &groups {
                        let tableau =
                            crate::formula::compile_residual(&group.core, Limits::default())
                                .expect("residual tableau");
                        let product = CompactProduct::with_policy(
                            &graph,
                            &tableau,
                            &labels,
                            product::Limits::default(),
                            AcceptancePolicy::Cells,
                        )
                        .expect("product");
                        for conditions in &group.alternatives {
                            found |= product
                                .find_lasso_with_conditions(&labels, conditions)
                                .expect("conditioned search")
                                .is_some();
                        }
                    }
                    assert_eq!(
                        found, expected,
                        "{formula:?}: bits={bits} len={len} back={back}"
                    );
                }
            }
        }
    }
}

#[test]
fn independent_strong_fairness_alternatives_share_one_memory_free_core() {
    let mut nodes = Vec::new();
    let mut conjuncts = Vec::new();
    for index in 0..3 {
        let start = nodes.len();
        nodes.push(Node::Atom(Atom {
            kind: AtomKind::State,
            index,
        }));
        nodes.push(Node::Always(start));
        nodes.push(Node::Eventually(start + 1));
        nodes.push(Node::Atom(Atom {
            kind: AtomKind::Action,
            index,
        }));
        nodes.push(Node::Eventually(start + 3));
        nodes.push(Node::Always(start + 4));
        nodes.push(Node::Or(vec![start + 2, start + 5]));
        conjuncts.push(start + 6);
    }
    nodes.push(Node::And(conjuncts));
    let root = nodes.len() - 1;
    let groups = decompose(
        &Formula::new(nodes, root).expect("three SF clauses"),
        Limits::default(),
    )
    .expect("decompose");
    assert_eq!(groups.len(), 1);
    assert_eq!(groups[0].alternatives.len(), 8);
    let tableau = crate::formula::compile_residual(&groups[0].core, Limits::default())
        .expect("memory-free tableau");
    assert_eq!(tableau.nodes.len(), 1);
    assert_eq!(tableau.promise_count, 0);
}

#[test]
fn solution_scratch_and_logical_cells_keep_existing_budget_boundaries() {
    let formula = Formula::new(
        vec![
            Node::Atom(Atom {
                kind: AtomKind::State,
                index: 0,
            }),
            Node::Eventually(0),
            Node::Always(1),
        ],
        2,
    )
    .expect("state recurrence");
    let groups = decompose(&formula, Limits::default()).expect("decompose");
    let graph = Graph::new(vec![vec![0]], vec![0]).expect("loop");
    let labels = Labels {
        states: vec![vec![true]],
        actions: vec![],
    };
    let tableau = crate::formula::compile_residual(&groups[0].core, Limits::default())
        .expect("memory-free tableau");
    let conditions = &groups[0].alternatives[0];
    let scratch = conditions.recurring[0].scratch_len();
    for (bytes, fits) in [(scratch - 1, false), (scratch, true)] {
        let product = CompactProduct::with_policy(
            &graph,
            &tableau,
            &labels,
            product::Limits::default(),
            AcceptancePolicy::Buffers {
                bytes,
                work: 10_000,
            },
        )
        .expect("empty core acceptance");
        let result = product.find_lasso_with_conditions(&labels, conditions);
        if fits {
            assert!(result.expect("exact scratch budget").is_some());
        } else {
            assert!(matches!(
                result,
                Err(product::ProductError::Limit("acceptance buffer bytes"))
            ));
        }
    }
    for (cells, fits) in [(1, false), (2, true)] {
        let product = CompactProduct::with_policy(
            &graph,
            &tableau,
            &labels,
            product::Limits {
                acceptance_cells: cells,
                ..product::Limits::default()
            },
            AcceptancePolicy::Cells,
        )
        .expect("core acceptance cells");
        let result = product.find_lasso_with_conditions(&labels, conditions);
        if fits {
            assert!(result.expect("exact logical cells").is_some());
        } else {
            assert!(matches!(
                result,
                Err(product::ProductError::Limit("acceptance cells"))
            ));
        }
    }
}

#[test]
fn pushing_atomic_negations_does_not_double_charge_the_original_input_limit() {
    let mut nodes = (0..1200)
        .map(|index| {
            Node::Atom(Atom {
                kind: AtomKind::State,
                index,
            })
        })
        .collect::<Vec<_>>();
    nodes.push(Node::Or((0..1200).collect()));
    nodes.push(Node::Not(1200));
    let original = Formula::new(nodes, 1201).expect("wide negated disjunction");
    let limits = Limits::default();
    assert!(matches!(
        decompose(
            &original,
            Limits {
                formula_nodes: original.terms() - 1,
                ..limits
            }
        ),
        Err(FormulaError::Limit("formula nodes"))
    ));
    decompose(
        &original,
        Limits {
            formula_nodes: original.terms(),
            ..limits
        },
    )
    .expect("exact original-input bound");
    let prior =
        crate::formula::compile(&original, limits).expect("original guard permits this input");
    let groups = decompose(&original, limits).expect("decompose within same input guard");
    assert_eq!(groups.len(), 1);
    let actual = crate::formula::compile_residual(&groups[0].core, limits)
        .expect("same normalized-node guard");
    assert_eq!(actual.nodes, prior.nodes);
    assert_eq!(actual.promise_count, prior.promise_count);
    assert!(matches!(
        crate::formula::compile_residual(
            &groups[0].core,
            Limits {
                formula_nodes: 1200,
                ..limits
            }
        ),
        Err(FormulaError::Limit("formula nodes"))
    ));
    crate::formula::compile_residual(
        &groups[0].core,
        Limits {
            formula_nodes: 1201,
            ..limits
        },
    )
    .expect("exact normalized-node bound is retained");
}

#[test]
fn an_expired_run_budget_cannot_produce_decomposition_results() {
    let formula = cases().pop().expect("formula");
    let watchdog = crate::budget::expired_for_test();
    let _scope = crate::budget::enter(watchdog.budget());
    assert!(matches!(
        decompose(&formula, Limits::default()),
        Err(FormulaError::Limit(crate::budget::EXHAUSTED))
    ));
}
