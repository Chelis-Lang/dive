//! Original-domain truth and witness controls for packed coordinator labels.
#![allow(clippy::expect_used, reason = "finite representation controls")]

use super::*;
use crate::formula::{self, Formula, Node};
use crate::product::{AcceptancePolicy, CompactProduct, Limits};

fn state(index: usize) -> Atom {
    Atom {
        kind: AtomKind::State,
        index,
    }
}
fn action(index: usize) -> Atom {
    Atom {
        kind: AtomKind::Action,
        index,
    }
}

fn copy(graph: &Graph, kinds: &[AtomKind], labels: &Labels) -> PackedTruth {
    let mut packed = PackedTruth::new(graph, kinds).expect("packed labels");
    for (atom, kind) in kinds.iter().enumerate() {
        for (v, edges) in graph.edges.iter().enumerate() {
            match kind {
                AtomKind::State => packed
                    .set_state(atom, v, labels.states[atom][v])
                    .expect("state"),
                AtomKind::Action => {
                    for e in 0..edges.len() {
                        packed
                            .set_action(atom, v, e, labels.actions[atom][v][e])
                            .expect("action");
                    }
                }
            }
        }
    }
    packed
}

#[test]
fn actual_domains_cross_word_boundaries_without_allocating_false_domains() {
    for vertices in [0, 1, 63, 64, 65, 127, 128, 129] {
        let edges = (0..vertices)
            .map(|v| vec![v, v, (v + 1) % vertices])
            .collect::<Vec<_>>();
        let graph = Graph::new(edges, if vertices == 0 { vec![] } else { vec![0] }).expect("graph");
        let kinds = [
            AtomKind::State,
            AtomKind::Action,
            AtomKind::State,
            AtomKind::Action,
        ];
        let labels = Labels {
            states: kinds
                .iter()
                .enumerate()
                .map(|(a, kind)| {
                    (0..vertices)
                        .map(|v| *kind == AtomKind::State && (a + v) % 3 == 0)
                        .collect()
                })
                .collect(),
            actions: kinds
                .iter()
                .enumerate()
                .map(|(a, kind)| {
                    graph
                        .edges
                        .iter()
                        .enumerate()
                        .map(|(v, row)| {
                            (0..row.len())
                                .map(|e| *kind == AtomKind::Action && (a + v + e) % 3 == 0)
                                .collect()
                        })
                        .collect()
                })
                .collect(),
        };
        let packed = copy(&graph, &kinds, &labels);
        packed.validate_dimensions(&graph).expect("shape");
        assert_eq!(packed.columns.len(), kinds.len());
        assert_eq!(packed.offsets.len(), vertices + 1);
        for (a, kind) in kinds.iter().enumerate() {
            let atom = Atom {
                kind: *kind,
                index: a,
            };
            assert!(packed.contains(atom));
            assert!(!packed.contains(Atom {
                kind: if *kind == AtomKind::State {
                    AtomKind::Action
                } else {
                    AtomKind::State
                },
                index: a
            }));
            let domain = if *kind == AtomKind::State {
                vertices
            } else {
                vertices * 3
            };
            assert_eq!(packed.columns[a].words.len(), words(domain));
            for (v, row) in graph.edges.iter().enumerate() {
                for e in 0..row.len() {
                    assert_eq!(packed.truth(atom, v, e), labels.truth(atom, v, e));
                }
            }
        }
        let layout = Layout::checked(vertices, vertices * 3, &kinds).expect("layout");
        let actual = packed.columns.capacity() * size_of::<Column>()
            + packed.offsets.capacity() * size_of::<usize>()
            + packed
                .columns
                .iter()
                .map(|column| column.words.capacity() * size_of::<u64>())
                .sum::<usize>();
        assert!(actual >= layout.minimum_bytes);
    }
}

fn compare_formula(
    graph: &Graph,
    formula: &Formula,
    labels: &Labels,
    packed: &PackedTruth,
    edge_count: usize,
) {
    let recurring = Formula::new(
        vec![
            Node::Atom(state(0)),
            Node::Eventually(0),
            Node::Always(1),
            Node::Atom(action(1)),
            Node::Eventually(3),
            Node::Always(4),
            Node::And(vec![2, 5]),
        ],
        6,
    )
    .expect("recurrences")
    .factor_recurrences(4096)
    .expect("factoring")
    .1;
    for group in formula::decompose(formula, formula::Limits::default()).expect("groups") {
        let tableau =
            formula::compile_residual(&group.core, formula::Limits::default()).expect("tableau");
        for recurrences in [&[][..], &recurring[..]] {
            for policy in [
                AcceptancePolicy::Legacy,
                AcceptancePolicy::Buffers {
                    bytes: 1_000_000,
                    work: 1_000_000,
                },
            ] {
                let original = CompactProduct::with_policy_and_edges(
                    graph,
                    &tableau,
                    labels,
                    recurrences,
                    Limits::default(),
                    policy,
                    edge_count,
                )
                .expect("original product");
                let actual = CompactProduct::with_policy_and_edges(
                    graph,
                    &tableau,
                    packed,
                    recurrences,
                    Limits::default(),
                    policy,
                    edge_count,
                )
                .expect("packed product");
                assert_eq!(
                    actual.parts.graph.owned_graph(),
                    original.parts.graph.owned_graph()
                );
                assert_eq!(actual.parts.behavior_states, original.parts.behavior_states);
                assert_eq!(
                    actual.parts.graph.owned_original_edges(),
                    original.parts.graph.owned_original_edges()
                );
                for conditions in &group.alternatives {
                    assert_eq!(
                        actual.find_lasso_with_conditions(packed, conditions),
                        original.find_lasso_with_conditions(labels, conditions)
                    );
                }
            }
        }
    }
}

#[test]
fn every_small_original_truth_assignment_keeps_products_and_conditioned_witnesses() {
    let nodes = vec![
        Node::Atom(state(0)),
        Node::Atom(action(1)),
        Node::Eventually(0),
        Node::Always(2),
        Node::Eventually(1),
        Node::Always(4),
        Node::Or(vec![0, 1]),
        Node::Always(6),
        Node::Eventually(7),
        Node::And(vec![3, 5]),
        Node::Or(vec![3, 5]),
        Node::And(vec![0, 4]),
        Node::Eventually(11),
        Node::Not(10),
    ];
    let formulas =
        [3, 5, 8, 9, 10, 12, 13].map(|root| Formula::new(nodes.clone(), root).expect("formula"));
    for mask in 0..16 {
        let edges = (0..2)
            .map(|v| {
                (0..2)
                    .filter(|to| mask & (1 << (v * 2 + to)) != 0)
                    .collect::<Vec<_>>()
            })
            .collect::<Vec<_>>();
        let graph = Graph::new(edges, vec![0]).expect("two-vertex graph");
        let edge_count = graph.edges.iter().map(Vec::len).sum::<usize>();
        for bits in 0..(1 << (2 + edge_count)) {
            let mut position = 2;
            let action_rows = graph
                .edges
                .iter()
                .map(|row| {
                    row.iter()
                        .map(|_| {
                            let truth = bits & (1 << position) != 0;
                            position += 1;
                            truth
                        })
                        .collect()
                })
                .collect::<Vec<_>>();
            let labels = Labels {
                states: vec![
                    (0..2).map(|v| bits & (1 << v) != 0).collect(),
                    vec![false; 2],
                ],
                actions: vec![
                    graph
                        .edges
                        .iter()
                        .map(|row| vec![false; row.len()])
                        .collect(),
                    action_rows,
                ],
            };
            let packed = copy(&graph, &[AtomKind::State, AtomKind::Action], &labels);
            for formula in &formulas {
                compare_formula(&graph, formula, &labels, &packed, edge_count);
            }
        }
    }
}

#[test]
fn malformed_references_dimensions_and_capacity_overflow_are_explicit_errors() {
    let graph = Graph::new(vec![vec![0, 0]], vec![0]).expect("parallel closing edges");
    let mut labels =
        PackedTruth::new(&graph, &[AtomKind::State, AtomKind::Action]).expect("labels");
    assert!(labels.set_state(1, 0, true).is_err());
    assert!(labels.set_action(0, 0, 0, true).is_err());
    assert!(labels.set_state(0, 1, true).is_err());
    assert!(labels.set_action(1, 0, 2, true).is_err());
    assert!(labels.set_action(1, usize::MAX, 0, true).is_err());
    assert_eq!(
        Layout::checked(usize::MAX, 0, &[AtomKind::Action]),
        Err(SIZE)
    );
    assert_eq!(
        Layout::checked(1, usize::MAX, &[AtomKind::Action; 128]),
        Err(SIZE)
    );
    assert!(matches!(reserve::<u64>(usize::MAX), Err(ALLOCATION)));
    labels.offsets[1] = 0;
    assert!(labels.validate_dimensions(&graph).is_err());
    labels.offsets[1] = 2;
    labels.columns[0].words.clear();
    assert!(labels.validate_dimensions(&graph).is_err());
}

#[test]
fn state_only_and_constant_plans_do_not_allocate_action_offsets() {
    let graph = Graph::new(vec![vec![0, 0]], vec![0]).expect("graph");
    for kinds in [vec![], vec![AtomKind::State]] {
        let labels = PackedTruth::new(&graph, &kinds).expect("state-only labels");
        assert_eq!(labels.offsets, [] as [usize; 0]);
        labels
            .validate_dimensions(&graph)
            .expect("state-only shape");
        let layout = Layout::checked(1, 2, &kinds).expect("layout");
        assert_eq!(layout.offsets, 0);
        assert_eq!(
            layout.minimum_bytes,
            kinds.len() * (size_of::<Column>() + size_of::<u64>())
        );
    }
}
