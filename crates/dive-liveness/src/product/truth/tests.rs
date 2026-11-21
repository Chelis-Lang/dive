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

fn copy<'a>(graph: &'a Graph, kinds: &[AtomKind], labels: &Labels) -> PackedTruth<'a> {
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
            + packed.offsets.len() * size_of::<usize>()
            + packed
                .columns
                .iter()
                .map(|column| column.words.capacity() * size_of::<u64>())
                .sum::<usize>();
        assert!(actual >= layout.dense_bytes);
    }
}

fn compare_formula(graph: &Graph, formula: &Formula, labels: &Labels, packed: &PackedTruth<'_>) {
    for group in formula::decompose(formula, formula::Limits::default()).expect("groups") {
        let tableau =
            formula::compile_residual(&group.core, formula::Limits::default()).expect("tableau");
        for policy in [
            AcceptancePolicy::Cells,
            AcceptancePolicy::Buffers {
                bytes: 1_000_000,
                work: 1_000_000,
            },
        ] {
            let original =
                CompactProduct::with_policy(graph, &tableau, labels, Limits::default(), policy)
                    .expect("original product");
            let actual =
                CompactProduct::with_policy(graph, &tableau, packed, Limits::default(), policy)
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
                compare_formula(&graph, formula, &labels, &packed);
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
    labels.offsets.to_mut()[1] = 0;
    assert!(labels.validate_dimensions(&graph).is_err());
    labels.offsets.to_mut()[1] = 2;
    labels.columns[0].words.clear();
    assert!(labels.validate_dimensions(&graph).is_err());
}

#[test]
fn state_only_and_constant_plans_do_not_allocate_action_offsets() {
    let graph = Graph::new(vec![vec![0, 0]], vec![0]).expect("graph");
    for kinds in [vec![], vec![AtomKind::State]] {
        let labels = PackedTruth::new(&graph, &kinds).expect("state-only labels");
        assert!(labels.offsets.is_empty());
        labels
            .validate_dimensions(&graph)
            .expect("state-only shape");
        let layout = Layout::checked(1, 2, &kinds).expect("layout");
        assert_eq!(layout.offsets, 0);
        assert_eq!(
            layout.dense_bytes,
            kinds.len() * (size_of::<Column>() + size_of::<u64>())
        );
    }
}

#[test]
fn native_truth_pages_preserve_every_physical_bit_and_borrow_offsets() {
    let edges = 10 * PAGE * 64 + 3;
    let offsets = [0, 63, 63, edges];
    let targets = vec![0; edges];
    let graph = crate::product::CsrTopology::new(&offsets, &targets, &[0]).expect("CSR");
    let mut packed = PackedTruth::new(
        &graph,
        &[AtomKind::State, AtomKind::Action, AtomKind::Action],
    )
    .expect("native truth");
    assert!(matches!(&packed.offsets, Cow::Borrowed(slice) if slice.as_ptr() == offsets.as_ptr()));
    let mut expected = vec![0u64; words(edges)];
    for (source, bounds) in offsets.windows(2).enumerate() {
        for position in bounds[0]..bounds[1] {
            packed
                .set_action(1, source, position - bounds[0], false)
                .expect("false evaluation");
        }
    }
    assert_eq!(packed.columns[1].words.capacity(), 0);
    assert_eq!(
        packed.columns[1]
            .pages
            .as_ref()
            .expect("paged")
            .directory
            .capacity(),
        0
    );

    // Out-of-order writes, a cleared final bit, adjacent pages and a zero row.
    for (position, value) in [
        (edges - 1, true),
        (4096, true),
        (63, true),
        (0, true),
        (4095, true),
        (4096, false),
        (edges - 2, true),
    ] {
        let source = if position < offsets[1] { 0 } else { 2 };
        packed
            .set_action(1, source, position - offsets[source], value)
            .expect("edge");
        let mask = 1 << (position % 64);
        if value {
            expected[position / 64] |= mask;
        } else {
            expected[position / 64] &= !mask;
        }
    }
    assert_eq!(packed.columns[1].words.len(), 3 * PAGE);
    assert!(
        packed
            .column_words(1, true)
            .expect("action")
            .eq(expected.iter().copied())
    );
    assert!(packed.column_words(1, false).is_none());
    for (source, bounds) in offsets.windows(2).enumerate() {
        for position in bounds[0]..bounds[1] {
            let edge = position - bounds[0];
            assert_eq!(
                packed.action(1, source, edge),
                expected[position / 64] & (1 << (position % 64)) != 0
            );
            assert!(!packed.action(2, source, edge));
            packed
                .set_action(2, source, edge, true)
                .expect("dense truth");
            assert!(packed.action(2, source, edge));
        }
    }
    let all = &packed.columns[2];
    let pages = all.pages.as_ref().expect("paged");
    assert_eq!(all.words.len(), pages.directory.len() * PAGE);
    assert!(all.words.capacity() <= pages.directory.len() * PAGE);
    assert_eq!(
        packed.column_words(2, true).expect("action").last(),
        Some(7)
    );
    packed
        .validate_dimensions(&graph)
        .expect("checked CSR shape");
}
