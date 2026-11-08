//! Dense and captured inputs share the same product and original-edge semantics.
#![allow(clippy::expect_used, reason = "finite topology differential controls")]

use super::*;
use crate::formula::{Atom, AtomKind, Tableau, TableauNode};
use crate::product::truth::TruthAccess;
use crate::product::{
    AcceptancePolicy, CompactProduct, CsrTopology, Labels, Limits, PackedTruth, Product,
    ProductError,
};
use crate::search::Lasso;
use dive_runtime::capture::{ConcreteSuccessor, Edge};
use std::sync::Arc;

fn captured(edges: Vec<Vec<usize>>, roots: Vec<usize>) -> BehaviorGraph {
    let states = edges
        .iter()
        .map(|_| Arc::from([Some(dive_value::Value::Int(0))]))
        .collect();
    let concrete = edges.iter().map(|_| Vec::new()).collect();
    BehaviorGraph {
        states,
        edges: edges
            .into_iter()
            .map(|row| {
                row.into_iter()
                    .map(|to| Edge {
                        to,
                        ordinal: None,
                        label: None,
                    })
                    .collect()
            })
            .collect(),
        concrete,
        roots,
    }
}

fn copied(graph: &BehaviorGraph) -> Result<Graph, GraphError> {
    Graph::new(
        graph
            .edges
            .iter()
            .map(|row| row.iter().map(|edge| edge.to).collect())
            .collect(),
        graph.roots.clone(),
    )
}

fn tableau() -> Tableau {
    Tableau {
        nodes: vec![TableauNode {
            literals: vec![],
            successors: vec![0],
            fulfilled: vec![true],
        }],
        roots: vec![0],
        promise_count: 1,
    }
}

#[derive(Debug, PartialEq, Eq)]
struct Snapshot {
    graph: Graph,
    original_edges: Vec<Vec<usize>>,
    behavior_states: Vec<usize>,
    state_acceptance: Vec<Vec<u64>>,
    materialization_work: usize,
    lasso: Option<Lasso>,
}

fn snapshot(
    graph: BehaviorView<'_>,
    labels: &impl TruthAccess,
    limits: Limits,
    policy: AcceptancePolicy,
) -> Result<Snapshot, ProductError> {
    let product = CompactProduct::with_policy(graph, &tableau(), labels, limits, policy)?;
    Ok(Snapshot {
        graph: product.parts.graph.owned_graph(),
        original_edges: product.parts.graph.owned_original_edges(),
        behavior_states: product.parts.behavior_states.clone(),
        state_acceptance: product.parts.acceptance.states.clone(),
        materialization_work: product.parts.acceptance.materialization_work,
        lasso: product.find_lasso()?,
    })
}

#[test]
fn dense_and_captured_inputs_preserve_full_products_acceptance_and_lassos() {
    for (edges, roots) in [
        (vec![], vec![]),
        (vec![vec![]], vec![0, 0]),
        (vec![vec![0, 0]], vec![0, 0]),
        (vec![vec![2, 1, 1], vec![1, 2], vec![0, 2]], vec![2, 0, 2]),
    ] {
        let captured = captured(edges, roots);
        let dense = copied(&captured).expect("dense reference");
        let view = BehaviorView::checked_captured(&captured).expect("borrowed topology");
        let labels = Labels {
            states: vec![],
            actions: vec![
                captured
                    .edges
                    .iter()
                    .map(|row| vec![true; row.len()])
                    .collect(),
            ],
        };
        for policy in [
            AcceptancePolicy::Legacy,
            AcceptancePolicy::Buffers {
                bytes: 1_000_000,
                work: 1_000_000,
            },
        ] {
            assert_eq!(
                snapshot(view, &labels, Limits::default(), policy),
                snapshot(
                    BehaviorView::Dense(&dense),
                    &labels,
                    Limits::default(),
                    policy
                )
            );
        }
    }
}

#[test]
fn every_existing_product_limit_keeps_its_dense_result() {
    let captured = captured(vec![vec![1, 0, 1], vec![1, 0]], vec![0, 1]);
    let dense = copied(&captured).expect("dense");
    let view = BehaviorView::checked_captured(&captured).expect("captured");
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![true; 3], vec![true; 2]]],
    };
    for ceiling in [0, 1, 2, 5, 100] {
        for limits in [
            Limits {
                vertices: ceiling,
                ..Limits::default()
            },
            Limits {
                edges: ceiling,
                ..Limits::default()
            },
            Limits {
                acceptance_cells: ceiling,
                ..Limits::default()
            },
            Limits {
                steps: ceiling,
                ..Limits::default()
            },
        ] {
            assert_eq!(
                snapshot(view, &labels, limits, AcceptancePolicy::Legacy),
                snapshot(
                    BehaviorView::Dense(&dense),
                    &labels,
                    limits,
                    AcceptancePolicy::Legacy
                )
            );
        }
        for policy in [
            AcceptancePolicy::Buffers {
                bytes: ceiling,
                work: 1_000_000,
            },
            AcceptancePolicy::Buffers {
                bytes: 1_000_000,
                work: ceiling,
            },
        ] {
            assert_eq!(
                snapshot(view, &labels, Limits::default(), policy),
                snapshot(
                    BehaviorView::Dense(&dense),
                    &labels,
                    Limits::default(),
                    policy
                )
            );
        }
    }
}

#[test]
fn packed_dimensions_offsets_and_truth_keep_all_original_indices() {
    for vertices in [0, 1, 63, 64, 65] {
        let captured = captured(
            (0..vertices)
                .map(|v| vec![v, v, (v + 1) % vertices])
                .collect(),
            (0..vertices).collect(),
        );
        let dense = copied(&captured).expect("dense");
        let view = BehaviorView::checked_captured(&captured).expect("borrowed");
        for kinds in [
            vec![],
            vec![AtomKind::State],
            vec![AtomKind::Action],
            vec![AtomKind::State, AtomKind::Action],
        ] {
            let mut original = PackedTruth::new(&dense, &kinds).expect("dense truth");
            let mut borrowed = PackedTruth::new(view, &kinds).expect("captured truth");
            for (atom, &kind) in kinds.iter().enumerate() {
                for v in 0..vertices {
                    match kind {
                        AtomKind::State => {
                            original
                                .set_state(atom, v, v % 2 == 0)
                                .expect("dense state");
                            borrowed
                                .set_state(atom, v, v % 2 == 0)
                                .expect("borrowed state");
                        }
                        AtomKind::Action => {
                            for edge in 0..view.row_len(v) {
                                original
                                    .set_action(atom, v, edge, edge % 2 == 0)
                                    .expect("dense edge");
                                borrowed
                                    .set_action(atom, v, edge, edge % 2 == 0)
                                    .expect("borrowed edge");
                            }
                        }
                    }
                    for edge in 0..view.row_len(v) {
                        let atom = Atom { kind, index: atom };
                        assert_eq!(original.truth(atom, v, edge), borrowed.truth(atom, v, edge));
                    }
                }
            }
            original
                .validate_dimensions(view)
                .expect("same borrowed dimensions");
            borrowed
                .validate_dimensions(&dense)
                .expect("same dense dimensions");
            let wrong = Graph::new(vec![], vec![]).expect("empty");
            assert_eq!(
                original.validate_dimensions(&wrong),
                borrowed.validate_dimensions(&wrong)
            );
        }
    }
}

#[test]
fn roots_then_destinations_use_edge_rows_not_captured_state_count() {
    let mut graph = captured(vec![vec![1, 9], vec![8]], vec![7, 6]);
    graph.states.clear();
    for expected in [
        GraphError {
            vertex: 7,
            source: None,
        },
        GraphError {
            vertex: 6,
            source: None,
        },
        GraphError {
            vertex: 9,
            source: Some(0),
        },
        GraphError {
            vertex: 8,
            source: Some(1),
        },
    ] {
        assert_eq!(copied(&graph).expect_err("dense rejects"), expected);
        assert_eq!(
            BehaviorView::checked_captured(&graph).expect_err("borrowed rejects"),
            expected
        );
        match expected.source {
            None => {
                graph.roots.remove(0);
            }
            Some(source) => {
                graph.edges[source].retain(|edge| edge.to != expected.vertex);
            }
        }
    }
    assert_eq!(
        BehaviorView::checked_captured(&graph)
            .expect("valid topology despite missing states")
            .vertex_count(),
        2
    );
}

#[test]
fn view_borrows_all_occurrences_without_cloning_state_owners() {
    let mut graph = captured(vec![vec![0, 0, 0]], vec![0, 0]);
    let original: dive_eval::next::StateVals = Arc::from([Some(dive_value::Value::Int(1))]);
    graph.concrete[0] = vec![
        ConcreteSuccessor {
            state: Arc::clone(&original),
            ordinal: 0,
            label: "first".into(),
        },
        ConcreteSuccessor {
            state: Arc::clone(&original),
            ordinal: 1,
            label: "duplicate".into(),
        },
    ];
    graph.edges[0][0].ordinal = Some(0);
    graph.edges[0][0].label = Some("first".into());
    graph.edges[0][1].ordinal = Some(1);
    graph.edges[0][1].label = Some("duplicate".into());
    let count = Arc::strong_count(&original);
    let state_count = Arc::strong_count(&graph.states[0]);
    let view = BehaviorView::checked_captured(&graph).expect("view");
    assert_eq!(view.roots().as_ptr(), graph.roots.as_ptr());
    assert_eq!(view.roots(), &[0, 0]);
    assert_eq!(view.row_len(0), 3);
    assert_eq!(view.target(0, 2), 0);
    assert_eq!(Arc::strong_count(&original), count);
    assert_eq!(Arc::strong_count(&graph.states[0]), state_count);
    assert_eq!(graph.concrete[0].len(), 2);
    assert_eq!(graph.concrete[0][0].ordinal, 0);
    assert_eq!(graph.concrete[0][1].label, "duplicate");
    assert_eq!(graph.edges[0][2].ordinal, None);
}

fn csr_arrays(graph: &Graph) -> (Vec<usize>, Vec<u32>) {
    let mut offsets = vec![0];
    let mut targets = Vec::new();
    for row in &graph.edges {
        targets.extend(
            row.iter()
                .map(|&to| u32::try_from(to).expect("small fixture")),
        );
        offsets.push(targets.len());
    }
    (offsets, targets)
}

#[test]
fn csr_preserves_owned_and_compact_products_and_original_edge_positions() {
    // All ordered two-vertex rows of length 0, 1 or 2: this includes parallel
    // edges, empty rows, unreachable vertices, self-loops and asymmetric cycles.
    let rows = [
        vec![],
        vec![0],
        vec![1],
        vec![0, 0],
        vec![0, 1],
        vec![1, 0],
        vec![1, 1],
    ];
    for first in &rows {
        for second in &rows {
            for roots in [vec![], vec![0], vec![1, 0, 1]] {
                let dense = Graph::new(vec![first.clone(), second.clone()], roots)
                    .expect("finite dense reference");
                let (offsets, targets) = csr_arrays(&dense);
                let csr = CsrTopology::new(&offsets, &targets, &dense.roots).expect("CSR");
                let labels = Labels {
                    states: vec![],
                    actions: vec![
                        dense
                            .edges
                            .iter()
                            .enumerate()
                            .map(|(v, row)| (0..row.len()).map(|e| (v + e) % 2 == 0).collect())
                            .collect(),
                    ],
                };
                let owned = Product::new(&dense, &tableau(), &labels, Limits::default());
                let borrowed = Product::from_csr(csr, &tableau(), &labels, Limits::default());
                assert_eq!(borrowed, owned);
                assert_eq!(
                    borrowed.expect("CSR owned product").find_lasso(),
                    owned.expect("dense owned product").find_lasso()
                );
                for policy in [
                    AcceptancePolicy::Legacy,
                    AcceptancePolicy::Buffers {
                        bytes: 1_000_000,
                        work: 1_000_000,
                    },
                ] {
                    assert_eq!(
                        snapshot((&csr).into(), &labels, Limits::default(), policy),
                        snapshot((&dense).into(), &labels, Limits::default(), policy)
                    );
                }
            }
        }
    }
}

#[test]
fn csr_keeps_every_existing_product_and_acceptance_limit() {
    let dense = Graph::new(vec![vec![1, 0, 1], vec![1, 0]], vec![0, 1]).expect("dense");
    let (offsets, targets) = csr_arrays(&dense);
    let csr = CsrTopology::new(&offsets, &targets, &dense.roots).expect("CSR");
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![true; 3], vec![true; 2]]],
    };
    for ceiling in [0, 1, 2, 5, 100] {
        for limits in [
            Limits {
                vertices: ceiling,
                ..Limits::default()
            },
            Limits {
                edges: ceiling,
                ..Limits::default()
            },
            Limits {
                acceptance_cells: ceiling,
                ..Limits::default()
            },
            Limits {
                steps: ceiling,
                ..Limits::default()
            },
        ] {
            assert_eq!(
                snapshot((&csr).into(), &labels, limits, AcceptancePolicy::Legacy),
                snapshot((&dense).into(), &labels, limits, AcceptancePolicy::Legacy)
            );
        }
        for policy in [
            AcceptancePolicy::Buffers {
                bytes: ceiling,
                work: 1_000_000,
            },
            AcceptancePolicy::Buffers {
                bytes: 1_000_000,
                work: ceiling,
            },
        ] {
            assert_eq!(
                snapshot((&csr).into(), &labels, Limits::default(), policy),
                snapshot((&dense).into(), &labels, Limits::default(), policy)
            );
        }
    }
}

#[test]
fn csr_keeps_shape_refusals_and_does_not_add_dead_end_stutters() {
    let graph = Graph::new(vec![vec![], vec![1]], vec![0]).expect("dead-end root");
    let (offsets, targets) = csr_arrays(&graph);
    let csr = CsrTopology::new(&offsets, &targets, &graph.roots).expect("CSR");
    for labels in [
        Labels {
            states: vec![vec![true]],
            actions: vec![],
        },
        Labels {
            states: vec![],
            actions: vec![vec![vec![]]],
        },
        Labels {
            states: vec![],
            actions: vec![vec![vec![true], vec![true]]],
        },
    ] {
        assert_eq!(
            Product::from_csr(csr, &tableau(), &labels, Limits::default()),
            Product::new(&graph, &tableau(), &labels, Limits::default())
        );
        assert!(Product::from_csr(csr, &tableau(), &labels, Limits::default()).is_err());
    }
    let labels = Labels {
        states: vec![],
        actions: vec![],
    };
    assert_eq!(
        Product::from_csr(csr, &tableau(), &labels, Limits::default())
            .expect("valid dead-end graph")
            .find_lasso()
            .expect("search"),
        None
    );
}

#[test]
fn csr_support_keeps_the_hot_topology_view_within_two_words() {
    assert!(size_of::<BehaviorView<'_>>() <= 2 * size_of::<usize>());
}
