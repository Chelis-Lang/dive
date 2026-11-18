//! Owned and compact acceptance use the same admitted product and witness.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "finite product assertions"
)]

use super::{
    AcceptancePolicy, CompactProduct, ImplicitProduct, Labels, Limits, Product, ProductError,
};
use crate::Graph;
use crate::formula::{
    self, Atom, AtomKind, CycleConditions, Formula, Node, Recurrence, Tableau, TableauNode,
};
use crate::search::{AcceptanceAccess, Lasso};

fn state() -> Node {
    Node::Atom(Atom {
        kind: AtomKind::State,
        index: 0,
    })
}

fn action() -> Node {
    Node::Atom(Atom {
        kind: AtomKind::Action,
        index: 0,
    })
}

fn tableau(nodes: Vec<Node>) -> Tableau {
    let root = nodes.len() - 1;
    formula::compile(
        &Formula::new(nodes, root).expect("formula"),
        formula::Limits::default(),
    )
    .expect("tableau")
}

fn recurrences() -> Vec<Recurrence> {
    let formula = Formula::new(
        vec![
            state(),
            Node::Eventually(0),
            Node::Always(1),
            action(),
            Node::Eventually(3),
            Node::Always(4),
            Node::And(vec![2, 5]),
        ],
        6,
    )
    .expect("recurring formula");
    // The bodies of `[]<>S /\ []<>A`, as solution decomposition extracts them.
    [0, 3]
        .map(|body| Recurrence::condition(&formula, &[body]).expect("recurrence"))
        .to_vec()
}

fn compare(
    graph: &Graph,
    tableau: &Tableau,
    labels: &Labels,
    limits: Limits,
) -> Result<Option<Lasso>, ProductError> {
    let owned = Product::new(graph, tableau, labels, limits);
    let view = CompactProduct::new(graph, tableau, labels, limits);
    assert_eq!(owned.as_ref().map(|_| ()), view.as_ref().map(|_| ()));
    let owned = owned?;
    let view = view?;
    let buffered = CompactProduct::with_policy(
        graph,
        tableau,
        labels,
        limits,
        AcceptancePolicy::compact_default(),
    )?;
    assert_eq!(view.find_lasso(), buffered.find_lasso());
    assert_eq!(
        view.parts.acceptance.states,
        buffered.parts.acceptance.states
    );
    assert_eq!(owned.graph, view.parts.graph.owned_graph());
    assert_eq!(owned.behavior_states, view.parts.behavior_states);
    assert_eq!(
        owned.original_edges,
        view.parts.graph.owned_original_edges()
    );
    assert_eq!(owned.acceptance.states.len(), view.state_count());
    assert_eq!(owned.acceptance.actions.len(), view.action_count());
    for (condition, row) in owned.acceptance.states.iter().enumerate() {
        for (v, &truth) in row.iter().enumerate() {
            assert_eq!(truth, view.state(condition, v));
        }
    }
    for (v, row) in owned.graph.edges.iter().enumerate() {
        for e in 0..row.len() {
            assert_eq!(owned.acceptance.recurrent[v][e], view.recurrent(v, e));
        }
    }
    let expected = owned.find_lasso()?;
    assert_eq!(expected, view.find_lasso()?);
    Ok(expected)
}

#[test]
fn compact_truth_and_witness_match_all_two_vertex_labelled_graphs() {
    let tableaux = [
        tableau(vec![Node::Bool(true)]),
        tableau(vec![state(), Node::Eventually(0), Node::Always(1)]),
    ];
    for edge_bits in 0..16 {
        let edges: Vec<Vec<usize>> = (0..2)
            .map(|v| {
                (0..2)
                    .filter(|t| edge_bits & (1 << (2 * v + t)) != 0)
                    .collect()
            })
            .collect();
        let graph = Graph::new(edges.clone(), vec![0]).expect("graph");
        for action_bits in 0..16 {
            let actions: Vec<Vec<bool>> = edges
                .iter()
                .enumerate()
                .map(|(v, row)| {
                    row.iter()
                        .map(|to| action_bits & (1 << (2 * v + to)) != 0)
                        .collect()
                })
                .collect();
            for state_bits in 0..4 {
                let labels = Labels {
                    states: vec![(0..2).map(|v| state_bits & (1 << v) != 0).collect()],
                    actions: vec![actions.clone()],
                };
                for t in &tableaux {
                    compare(&graph, t, &labels, Limits::default())
                        .expect("admitted finite product");
                }
            }
        }
    }
}

#[test]
fn compact_preserves_prefix_parallel_and_closing_edges() {
    let graph = Graph::new(vec![vec![1], vec![2, 2], vec![1], vec![3]], vec![0])
        .expect("prefix, duplicate edges, unreachable component");
    let labels = Labels {
        states: vec![vec![false; 4]],
        actions: vec![vec![
            vec![false],
            vec![false, true],
            vec![false],
            vec![true],
        ]],
    };
    let t = tableau(vec![Node::Bool(true)]);
    compare(&graph, &t, &labels, Limits::default()).expect("product");
    // `[]<>A` is a cycle condition of the solution, evaluated during the search.
    let solution = CycleConditions {
        always: vec![],
        recurring: recurrences()[1..].to_vec(),
    };
    let view = CompactProduct::new(&graph, &t, &labels, Limits::default()).expect("view");
    let lasso = view
        .find_lasso_with_conditions(&labels, &solution)
        .expect("search")
        .expect("accepting cycle");
    assert_eq!(lasso.prefix.len(), 1);
    assert_eq!((lasso.prefix[0].from, lasso.prefix[0].to), (0, 1));
    assert_eq!(lasso.cycle.len(), 2);
    assert_eq!(
        (lasso.cycle[0].from, lasso.cycle[0].edge, lasso.cycle[0].to),
        (1, 1, 2)
    );
    assert_eq!((lasso.cycle[1].from, lasso.cycle[1].to), (2, 1));
    for policy in [AcceptancePolicy::Cells, AcceptancePolicy::compact_default()] {
        let implicit = ImplicitProduct::new(&graph, &t, &labels, Limits::default(), policy)
            .expect("on-the-fly product");
        assert_eq!(
            implicit.find_lasso_with_conditions(&labels, &solution),
            Ok(Some(lasso.clone()))
        );
    }
    assert_eq!(
        view.find_lasso_with_conditions(&labels, &solution),
        Ok(Some(lasso))
    );
}

#[test]
fn hashed_pair_discovery_keeps_exact_cartesian_rows_and_original_edge_positions() {
    let graph = Graph::new(vec![vec![2, 1, 1], vec![0, 2], vec![1, 0]], vec![2, 0])
        .expect("reverse roots, duplicate and reverse targets");
    let t = Tableau {
        nodes: (0..3)
            .map(|_| TableauNode {
                literals: vec![],
                successors: vec![2, 0, 1],
                fulfilled: vec![true],
            })
            .collect(),
        roots: vec![2, 1, 0],
        promise_count: 1,
    };
    let labels = Labels {
        states: vec![],
        actions: vec![],
    };
    let expected_rows: Vec<Vec<usize>> = graph
        .edges
        .iter()
        .flat_map(|row| {
            (0..3).map(move |_| {
                row.iter()
                    .flat_map(|target| [target * 3 + 2, target * 3, target * 3 + 1])
                    .collect()
            })
        })
        .collect();
    let expected_originals: Vec<Vec<usize>> = graph
        .edges
        .iter()
        .flat_map(|row| (0..3).map(move |_| (0..row.len()).flat_map(|edge| [edge; 3]).collect()))
        .collect();
    // A new randomized lookup table is constructed for every repetition.
    // Its bucket order must affect neither public nor compact products.
    for _ in 0..8 {
        let owned = Product::new(&graph, &t, &labels, Limits::default()).expect("product");
        assert_eq!(owned.graph.edges, expected_rows);
        assert_eq!(owned.graph.roots, [0, 1, 2, 6, 7, 8]);
        assert_eq!(owned.behavior_states, [0, 0, 0, 1, 1, 1, 2, 2, 2]);
        assert_eq!(owned.original_edges, expected_originals);
        assert!(owned.acceptance.states.iter().flatten().all(|value| *value));
        compare(&graph, &t, &labels, Limits::default()).expect("matching compact product");
    }
}

#[test]
fn compact_empty_dead_end_and_unreachable_languages_match_owned() {
    for (edges, roots) in [
        (vec![], vec![]),
        (vec![vec![]], vec![0]),
        (vec![vec![0]], vec![]),
        (vec![vec![], vec![1]], vec![0]),
    ] {
        let labels = Labels {
            states: vec![vec![false; edges.len()]],
            actions: vec![edges.iter().map(|row| vec![false; row.len()]).collect()],
        };
        let graph = Graph::new(edges, roots).expect("graph");
        for t in [
            tableau(vec![Node::Bool(true)]),
            tableau(vec![Node::Bool(false)]),
            tableau(vec![state(), Node::Eventually(0)]),
        ] {
            assert_eq!(compare(&graph, &t, &labels, Limits::default()), Ok(None));
        }
    }
}

#[test]
fn compact_preserves_custom_limit_boundaries_and_shape_error_priority() {
    let graph = Graph::new(vec![vec![1, 1], vec![0]], vec![0]).expect("graph");
    let labels = Labels {
        states: vec![vec![true, false]],
        actions: vec![vec![vec![false, true], vec![true]]],
    };
    let t = tableau(vec![state(), Node::Eventually(0)]);
    for value in 0..=64 {
        for limits in [
            Limits {
                vertices: value,
                ..Limits::default()
            },
            Limits {
                edges: value,
                ..Limits::default()
            },
            Limits {
                acceptance_cells: value,
                ..Limits::default()
            },
            Limits {
                steps: value,
                ..Limits::default()
            },
        ] {
            let _ = compare(&graph, &t, &labels, limits);
        }
    }
    let zero = Limits {
        vertices: 0,
        edges: 0,
        acceptance_cells: 0,
        steps: 0,
    };
    assert_eq!(
        compare(&graph, &t, &labels, zero),
        Err(ProductError::Limit("transition steps"))
    );
    let mut bad = labels.clone();
    bad.states[0].clear();
    assert_eq!(
        compare(&graph, &t, &bad, zero),
        Err(ProductError::Shape("state predicate dimensions".into()))
    );
    let mut bad = t.clone();
    bad.roots.push(bad.nodes.len());
    assert_eq!(
        compare(&graph, &bad, &labels, zero),
        Err(ProductError::Shape(
            "tableau root outside node table".into()
        ))
    );
    let dead = Graph::new(vec![vec![]], vec![0]).expect("dead end");
    let labels = Labels {
        states: vec![vec![false]],
        actions: vec![],
    };
    assert_eq!(
        compare(
            &dead,
            &t,
            &labels,
            Limits {
                acceptance_cells: 0,
                ..Limits::default()
            }
        ),
        Err(ProductError::Limit("acceptance cells"))
    );
}

#[test]
fn cycle_predicates_outside_the_label_table_are_shape_errors() {
    let graph = Graph::new(vec![vec![1, 1], vec![0]], vec![0]).expect("graph");
    let labels = Labels {
        states: vec![vec![true, false]],
        actions: vec![vec![vec![false, true], vec![true]]],
    };
    let t = tableau(vec![state(), Node::Eventually(0)]);
    let invalid = Formula::new(
        vec![
            Node::Atom(Atom {
                kind: AtomKind::Action,
                index: 1,
            }),
            Node::Eventually(0),
            Node::Always(1),
        ],
        2,
    )
    .expect("formula");
    let invalid = CycleConditions {
        always: vec![],
        recurring: vec![Recurrence::condition(&invalid, &[0]).expect("recurrence")],
    };
    let cycle_shape = Err(ProductError::Shape(
        "cycle predicate outside label table".into(),
    ));
    let view = CompactProduct::new(&graph, &t, &labels, Limits::default()).expect("view");
    assert_eq!(
        view.find_lasso_with_conditions(&labels, &invalid),
        cycle_shape
    );
    for policy in [AcceptancePolicy::Cells, AcceptancePolicy::compact_default()] {
        let implicit = ImplicitProduct::new(&graph, &t, &labels, Limits::default(), policy)
            .expect("on-the-fly product");
        assert_eq!(
            implicit.find_lasso_with_conditions(&labels, &invalid),
            cycle_shape
        );
    }
}

#[test]
fn compact_capacity_packs_promises_without_recurrent_matrices() {
    let graph = Graph::new(vec![vec![0]; 64], (0..64).collect()).expect("graph");
    let labels = Labels {
        states: vec![],
        actions: vec![],
    };
    for promises in 0..=1 {
        let t = Tableau {
            nodes: vec![TableauNode {
                literals: vec![],
                successors: vec![0],
                fulfilled: vec![true; promises],
            }],
            roots: vec![0],
            promise_count: promises,
        };
        let owned = Product::new(&graph, &t, &labels, Limits::default()).expect("owned");
        let view = CompactProduct::new(&graph, &t, &labels, Limits::default()).expect("compact");
        let packed_bytes = view
            .parts
            .acceptance
            .states
            .iter()
            .map(|row| row.capacity() * size_of::<u64>())
            .sum::<usize>();
        let promise_bytes: usize = owned
            .acceptance
            .states
            .iter()
            .map(|row| row.capacity() * size_of::<bool>())
            .sum();
        if promises == 0 {
            assert_eq!(packed_bytes, 0);
        } else {
            assert_eq!(packed_bytes, 8);
            assert_eq!(promise_bytes, 64);
        }
        assert_eq!(owned.find_lasso(), view.find_lasso());
        eprintln!(
            "promises={promises} packed_promise_bytes={packed_bytes} owned_promise_payload_bytes={promise_bytes}"
        );
    }
}

#[test]
fn compact_bits_cross_word_boundaries_with_empty_rows_and_parallel_edges() {
    let graph = Graph::new(
        (0..70)
            .map(|v| if v % 7 == 0 { vec![] } else { vec![v, v] })
            .collect(),
        (0..70).collect(),
    )
    .expect("graph");
    let t = Tableau {
        nodes: vec![
            TableauNode {
                literals: vec![],
                successors: vec![1],
                fulfilled: vec![true, false],
            },
            TableauNode {
                literals: vec![],
                successors: vec![0],
                fulfilled: vec![false, true],
            },
        ],
        roots: vec![0],
        promise_count: 2,
    };
    let labels = Labels {
        states: vec![(0..70).map(|v| v % 2 == 0).collect()],
        actions: vec![
            graph
                .edges
                .iter()
                .enumerate()
                .map(|(v, row)| {
                    row.iter()
                        .enumerate()
                        .map(|(e, _)| (v + e) % 2 == 0)
                        .collect()
                })
                .collect(),
        ],
    };
    compare(&graph, &t, &labels, Limits::default()).expect("equivalent");
    let view = CompactProduct::new(&graph, &t, &labels, Limits::default()).expect("view");
    let vertices = view.parts.graph.owned_graph().edges.len();
    let edges: usize = view
        .parts
        .graph
        .owned_graph()
        .edges
        .iter()
        .map(Vec::len)
        .sum();
    assert!(vertices > 64 && edges > 128);
    assert!(
        view.parts
            .acceptance
            .states
            .iter()
            .all(|row| row.len() == vertices.div_ceil(64))
    );
}
