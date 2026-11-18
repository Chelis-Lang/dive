//! The on-the-fly product preserves edge semantics and agrees with direct
//! word truth. Limit controls use the legacy logical-cell policy; the word
//! sweeps also check Core's buffer policy.
#![allow(clippy::expect_used, clippy::panic, reason = "test assertions")]

use super::{AcceptancePolicy, ImplicitProduct, Labels, Limits, ProductError};
use crate::Graph;
use crate::formula::{
    Atom, AtomKind, CycleConditions, Formula, Limits as FormulaLimits, Literal, Node, Tableau,
    TableauNode, compile,
};
use crate::search::Lasso;

/// The temporal route's product of a dense test graph, searched with no
/// cycle conditions.
struct OnTheFly<'a> {
    product: ImplicitProduct<'a, Labels>,
    labels: &'a Labels,
}

impl<'a> OnTheFly<'a> {
    fn new(
        graph: &'a Graph,
        tableau: &'a Tableau,
        labels: &'a Labels,
        limits: Limits,
    ) -> Result<Self, ProductError> {
        Self::with_policy(graph, tableau, labels, limits, AcceptancePolicy::Cells)
    }

    fn with_policy(
        graph: &'a Graph,
        tableau: &'a Tableau,
        labels: &'a Labels,
        limits: Limits,
        policy: AcceptancePolicy,
    ) -> Result<Self, ProductError> {
        let product = ImplicitProduct::new(graph, tableau, labels, limits, policy)?;
        Ok(Self { product, labels })
    }

    fn find_lasso(&self) -> Result<Option<Lasso>, ProductError> {
        let none = CycleConditions {
            always: vec![],
            recurring: vec![],
        };
        self.product.find_lasso_with_conditions(self.labels, &none)
    }
}

fn formula(nodes: Vec<Node>) -> Tableau {
    let root = nodes.len() - 1;
    compile(
        &Formula::new(nodes, root).expect("formula"),
        FormulaLimits::default(),
    )
    .expect("tableau")
}

fn atom(kind: AtomKind) -> Node {
    Node::Atom(Atom { kind, index: 0 })
}

#[test]
fn action_truth_is_per_edge_and_includes_the_closing_transition() {
    let graph = Graph::new(vec![vec![1, 0], vec![0]], vec![0]).expect("graph");
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![true, false], vec![true]]],
    };
    let tableau = formula(vec![atom(AtomKind::Action), Node::Always(0)]);
    let product = OnTheFly::new(&graph, &tableau, &labels, Limits::default()).expect("product");
    let lasso = product
        .find_lasso()
        .expect("search")
        .expect("alternating cycle");
    assert_eq!(lasso.root, 0);
    assert_eq!(
        lasso
            .cycle
            .iter()
            .map(|s| (s.from, s.edge, s.to))
            .collect::<Vec<_>>(),
        [(0, 0, 1), (1, 0, 0)]
    );
    let mut broken = labels.clone();
    broken.actions[0][1][0] = false;
    assert!(
        OnTheFly::new(&graph, &tableau, &broken, Limits::default())
            .expect("product")
            .find_lasso()
            .expect("search")
            .is_none()
    );
}

#[test]
fn parallel_edge_positions_are_preserved_and_dead_ends_are_not_stutters() {
    let graph = Graph::new(vec![vec![0, 0]], vec![0]).expect("graph");
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![false, true]]],
    };
    let tableau = formula(vec![atom(AtomKind::Action), Node::Always(0)]);
    let lasso = OnTheFly::new(&graph, &tableau, &labels, Limits::default())
        .expect("product")
        .find_lasso()
        .expect("search")
        .expect("second edge");
    assert_eq!(lasso.cycle[0].edge, 1);
    let dead = Graph::new(vec![vec![]], vec![0]).expect("dead end");
    assert!(
        OnTheFly::new(
            &dead,
            &formula(vec![Node::Bool(true)]),
            &Labels {
                states: vec![],
                actions: vec![]
            },
            Limits::default()
        )
        .expect("product")
        .find_lasso()
        .expect("search")
        .is_none()
    );
}

#[test]
fn reachable_pairs_fit_small_budgets_without_ignoring_invalid_inputs() {
    let graph = Graph::new((0..10_000).map(|i| vec![i]).collect(), vec![0])
        .expect("mostly unreachable behavior vertices");
    let mut tableau = Tableau {
        nodes: (0..200)
            .map(|i| TableauNode {
                literals: vec![],
                successors: vec![i],
                fulfilled: vec![true],
            })
            .collect(),
        roots: vec![0],
        promise_count: 1,
    };
    let labels = Labels {
        states: vec![],
        actions: vec![],
    };
    let limits = Limits {
        vertices: 1,
        edges: 1,
        acceptance_cells: 2,
        steps: 4,
    };
    let lasso = OnTheFly::new(&graph, &tableau, &labels, limits)
        .expect("one reachable pair, not two million Cartesian pairs")
        .find_lasso()
        .expect("search")
        .expect("self loop");
    assert_eq!(
        (
            lasso.root,
            lasso.entry,
            lasso.prefix.len(),
            lasso.cycle.len()
        ),
        (0, 0, 0, 1)
    );
    assert_eq!(
        (lasso.cycle[0].from, lasso.cycle[0].edge, lasso.cycle[0].to),
        (0, 0, 0)
    );
    tableau.nodes[199].successors = vec![200];
    assert!(matches!(
        OnTheFly::new(&graph, &tableau, &labels, limits),
        Err(ProductError::Shape(_))
    ));
    tableau.nodes[199].successors = vec![199];
    tableau.nodes[199].literals.push(Literal {
        atom: Atom {
            kind: AtomKind::State,
            index: 0,
        },
        positive: true,
    });
    assert!(matches!(
        OnTheFly::new(&graph, &tableau, &labels, limits),
        Err(ProductError::Shape(_))
    ));
}

#[test]
fn pair_discovery_order_does_not_choose_the_projected_witness() {
    let graph = Graph::new(vec![vec![0, 0], vec![1, 1]], vec![1, 0]).expect("reverse roots");
    let tableau = Tableau {
        nodes: [true, false]
            .into_iter()
            .enumerate()
            .map(|(i, positive)| TableauNode {
                literals: vec![Literal {
                    atom: Atom {
                        kind: AtomKind::Action,
                        index: 0,
                    },
                    positive,
                }],
                successors: vec![i],
                fulfilled: vec![],
            })
            .collect(),
        roots: vec![1, 0],
        promise_count: 0,
    };
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![false, true], vec![false, true]]],
    };
    let lasso = OnTheFly::new(&graph, &tableau, &labels, Limits::default())
        .expect("product")
        .find_lasso()
        .expect("search")
        .expect("loop");
    assert_eq!((lasso.root, lasso.entry), (0, 0));
    // Discovery starts with (1,1), but the canonical entry is (0,0),
    // whose action literal requires the second original parallel edge.
    assert_eq!(lasso.cycle.len(), 1);
    assert_eq!(
        (lasso.cycle[0].from, lasso.cycle[0].edge, lasso.cycle[0].to),
        (0, 1, 0)
    );
}

#[test]
fn reachable_growth_cannot_exceed_vertices_or_promise_cells_silently() {
    let graph = Graph::new(vec![vec![1], vec![1]], vec![0]).expect("reachable successor");
    let tableau = Tableau {
        nodes: vec![TableauNode {
            literals: vec![],
            successors: vec![0],
            fulfilled: vec![true],
        }],
        roots: vec![0],
        promise_count: 1,
    };
    let labels = Labels {
        states: vec![],
        actions: vec![],
    };
    for (limits, kind) in [
        (
            Limits {
                vertices: 1,
                ..Limits::default()
            },
            "vertices",
        ),
        (
            Limits {
                acceptance_cells: 3,
                ..Limits::default()
            },
            "acceptance cells",
        ),
    ] {
        assert!(matches!(
            OnTheFly::new(&graph, &tableau, &labels, limits),
            Err(ProductError::Limit(found)) if found == kind
        ));
    }
    let lasso = OnTheFly::new(
        &graph,
        &tableau,
        &labels,
        Limits {
            vertices: 2,
            edges: 2,
            acceptance_cells: 4,
            ..Limits::default()
        },
    )
    .expect("exact entry budgets")
    .find_lasso()
    .expect("search")
    .expect("reachable loop");
    assert_eq!((lasso.root, lasso.entry, lasso.prefix.len()), (0, 1, 1));
}

fn reachable(start: usize, len: usize, back: usize) -> Vec<usize> {
    let mut seen = vec![];
    let mut i = start;
    while !seen.contains(&i) {
        seen.push(i);
        i = if i + 1 == len { back } else { i + 1 };
    }
    seen
}

#[test]
fn every_short_two_atom_word_agrees_with_direct_semantics() {
    // State and action bits are independent: the closing action is NOT
    // inferred from the state label or from a different outgoing edge.
    let cases = vec![
        vec![atom(AtomKind::State), Node::Always(0)],
        vec![atom(AtomKind::Action), Node::Always(0)],
        vec![atom(AtomKind::State), Node::Eventually(0), Node::Always(1)],
        vec![atom(AtomKind::Action), Node::Always(0), Node::Eventually(1)],
        vec![
            atom(AtomKind::State),
            atom(AtomKind::Action),
            Node::Eventually(0),
            Node::Eventually(1),
            Node::And(vec![2, 3]),
            Node::Always(4),
        ],
    ];
    for (case, nodes) in cases.into_iter().enumerate() {
        let tableau = formula(nodes);
        for len in 1..=4 {
            for bits in 0..(1_u32 << (2 * len)) {
                let state: Vec<bool> = (0..len).map(|i| bits & (1 << (2 * i)) != 0).collect();
                let action: Vec<bool> = (0..len).map(|i| bits & (2 << (2 * i)) != 0).collect();
                for back in 0..len {
                    let edges: Vec<Vec<usize>> = (0..len)
                        .map(|i| vec![if i + 1 == len { back } else { i + 1 }])
                        .collect();
                    let graph = Graph::new(edges.clone(), vec![0]).expect("word");
                    let labels = Labels {
                        states: vec![state.clone()],
                        actions: vec![action.iter().map(|b| vec![*b]).collect()],
                    };
                    let product = OnTheFly::new(&graph, &tableau, &labels, Limits::default())
                        .expect("product");
                    let expected = match case {
                        0 => state.iter().all(|b| *b),
                        1 => action.iter().all(|b| *b),
                        2 => state[back..].iter().any(|b| *b),
                        3 => action[back..].iter().all(|b| *b),
                        _ => state[back..].iter().any(|b| *b) && action[back..].iter().any(|b| *b),
                    };
                    let lasso = product.find_lasso().expect("search");
                    // Core's buffer policy finds the same witness.
                    assert_eq!(
                        OnTheFly::with_policy(
                            &graph,
                            &tableau,
                            &labels,
                            Limits::default(),
                            AcceptancePolicy::compact_default(),
                        )
                        .expect("buffered product")
                        .find_lasso()
                        .expect("buffered search"),
                        lasso
                    );
                    assert_eq!(
                        lasso.is_some(),
                        expected,
                        "case {case}, bits {bits}, len {len}, back {back}"
                    );
                    if let Some(lasso) = lasso {
                        let mut at = lasso.root;
                        for step in lasso.prefix.iter().chain(&lasso.cycle) {
                            assert_eq!(step.from, at);
                            assert_eq!(edges[at][step.edge], step.to);
                            at = step.to;
                        }
                        assert_eq!(at, lasso.entry);
                        // Projection must not discard repeated positions with
                        // different obligations. Direct word semantics at the
                        // projected cycle provides an independent witness check.
                        let positions: Vec<usize> = lasso
                            .prefix
                            .iter()
                            .chain(&lasso.cycle)
                            .map(|s| s.from)
                            .collect();
                        let loop_at = lasso.prefix.len();
                        let future = |i| reachable(i, positions.len(), loop_at);
                        let witness = match case {
                            0 => positions.iter().all(|v| state[*v]),
                            1 => positions.iter().all(|v| action[*v]),
                            2 => future(loop_at).iter().any(|i| state[positions[*i]]),
                            3 => future(loop_at).iter().all(|i| action[positions[*i]]),
                            _ => {
                                future(loop_at).iter().any(|i| state[positions[*i]])
                                    && future(loop_at).iter().any(|i| action[positions[*i]])
                            }
                        };
                        assert!(witness);
                    }
                }
            }
        }
    }
}

#[test]
fn malformed_labels_and_limits_are_errors_never_absence_of_acceptance() {
    let graph = Graph::new(vec![vec![0]], vec![0]).expect("graph");
    let tableau = formula(vec![atom(AtomKind::Action), Node::Always(0)]);
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![true]]],
    };
    for limits in [
        Limits {
            acceptance_cells: 0,
            ..Limits::default()
        },
        Limits {
            vertices: 0,
            ..Limits::default()
        },
        Limits {
            edges: 0,
            ..Limits::default()
        },
        Limits {
            steps: 0,
            ..Limits::default()
        },
    ] {
        assert!(matches!(
            OnTheFly::new(&graph, &tableau, &labels, limits),
            Err(ProductError::Limit(_))
        ));
    }
    let mut bad = labels.clone();
    bad.actions[0][0].clear();
    assert!(matches!(
        OnTheFly::new(&graph, &tableau, &bad, Limits::default()),
        Err(ProductError::Shape(_))
    ));
    let mut bad = tableau.clone();
    bad.roots.push(bad.nodes.len());
    assert!(matches!(
        OnTheFly::new(&graph, &bad, &labels, Limits::default()),
        Err(ProductError::Shape(_))
    ));
    let mut bad = tableau.clone();
    bad.nodes[0].fulfilled.push(true);
    assert!(matches!(
        OnTheFly::new(&graph, &bad, &labels, Limits::default()),
        Err(ProductError::Shape(_))
    ));
}

fn closure(edges: &[Vec<usize>], actions: &[Vec<bool>], restricted: bool) -> Vec<Vec<bool>> {
    let n = edges.len();
    let mut paths = vec![vec![false; n]; n];
    for i in 0..n {
        paths[i][i] = true;
        for (e, &j) in edges[i].iter().enumerate() {
            if !restricted || actions[i][e] {
                paths[i][j] = true;
            }
        }
    }
    for k in 0..n {
        for i in 0..n {
            for j in 0..n {
                paths[i][j] |= paths[i][k] && paths[k][j];
            }
        }
    }
    paths
}

#[test]
fn all_two_vertex_branching_graphs_match_independent_path_closure() {
    let tableaux = [
        formula(vec![atom(AtomKind::Action), Node::Always(0)]),
        formula(vec![
            atom(AtomKind::Action),
            Node::Always(0),
            Node::Eventually(1),
        ]),
        formula(vec![
            atom(AtomKind::State),
            Node::Eventually(0),
            Node::Always(1),
        ]),
        formula(vec![
            atom(AtomKind::State),
            atom(AtomKind::Action),
            Node::Eventually(0),
            Node::Eventually(1),
            Node::And(vec![2, 3]),
            Node::Always(4),
        ]),
    ];
    for edge_bits in 0..16 {
        let edges: Vec<Vec<usize>> = (0..2)
            .map(|v| {
                (0..2)
                    .filter(|t| edge_bits & (1 << (2 * v + t)) != 0)
                    .collect()
            })
            .collect();
        for action_bits in 0..16 {
            let actions: Vec<Vec<bool>> = edges
                .iter()
                .enumerate()
                .map(|(v, ts)| {
                    ts.iter()
                        .map(|t| action_bits & (1 << (2 * v + t)) != 0)
                        .collect()
                })
                .collect();
            let full = closure(&edges, &actions, false);
            let only_true = closure(&edges, &actions, true);
            for state_bits in 0..4 {
                let states: Vec<bool> = (0..2).map(|v| state_bits & (1 << v) != 0).collect();
                let graph = Graph::new(edges.clone(), vec![0]).expect("graph");
                let labels = Labels {
                    states: vec![states.clone()],
                    actions: vec![actions.clone()],
                };
                let true_cycle = |v: usize| {
                    edges[v]
                        .iter()
                        .enumerate()
                        .any(|(e, t)| actions[v][e] && only_true[*t][v])
                };
                let full_cycle = |v: usize| edges[v].iter().any(|t| full[*t][v]);
                let expected = [
                    (0..2).any(|v| only_true[0][v] && true_cycle(v)),
                    (0..2).any(|v| full[0][v] && true_cycle(v)),
                    (0..2).any(|v| full[0][v] && states[v] && full_cycle(v)),
                    (0..2).any(|v| {
                        full[0][v]
                            && edges[v].iter().enumerate().any(|(e, t)| {
                                actions[v][e]
                                    && full[*t][v]
                                    && (0..2).any(|s| states[s] && full[v][s] && full[s][v])
                            })
                    }),
                ];
                for (t, want) in tableaux.iter().zip(expected) {
                    let product =
                        OnTheFly::new(&graph, t, &labels, Limits::default()).expect("product");
                    let lasso = product.find_lasso().expect("search");
                    assert_eq!(
                        lasso.is_some(),
                        want,
                        "edges {edge_bits}, actions {action_bits}, states {state_bits}"
                    );
                    let buffered = OnTheFly::with_policy(
                        &graph,
                        t,
                        &labels,
                        Limits::default(),
                        AcceptancePolicy::compact_default(),
                    )
                    .expect("buffered product");
                    assert_eq!(buffered.find_lasso().expect("buffered search"), lasso);
                }
            }
        }
    }
}

#[test]
fn empty_language_and_promise_memory_are_distinguished() {
    let graph = Graph::new(vec![vec![]], vec![0]).expect("graph");
    let labels = Labels {
        states: vec![vec![false]],
        actions: vec![],
    };
    let eventuality = formula(vec![atom(AtomKind::State), Node::Eventually(0)]);
    assert!(matches!(
        OnTheFly::new(
            &graph,
            &eventuality,
            &labels,
            Limits {
                acceptance_cells: 0,
                ..Limits::default()
            }
        ),
        Err(ProductError::Limit("acceptance cells"))
    ));
    let false_tableau = formula(vec![Node::Bool(false)]);
    assert!(
        OnTheFly::new(&graph, &false_tableau, &labels, Limits::default())
            .expect("empty-language product")
            .find_lasso()
            .expect("search")
            .is_none()
    );
}

#[test]
fn borrowed_csr_self_loop_accepts_the_true_formula() {
    let offsets = [0, 1];
    let targets = [0];
    let roots = [0];
    let topology = super::CsrTopology::new(&offsets, &targets, &roots).expect("topology");
    let tableau = formula(vec![Node::Bool(true)]);
    let labels = Labels {
        states: vec![],
        actions: vec![],
    };
    let product = ImplicitProduct::new(
        &topology,
        &tableau,
        &labels,
        Limits::default(),
        AcceptancePolicy::Cells,
    )
    .expect("product");
    let none = CycleConditions {
        always: vec![],
        recurring: vec![],
    };
    assert!(
        product
            .find_lasso_with_conditions(&labels, &none)
            .expect("search")
            .is_some()
    );
}
