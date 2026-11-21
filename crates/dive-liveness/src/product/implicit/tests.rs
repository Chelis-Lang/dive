//! The on-the-fly product admits, orders and searches exactly as the
//! materialized reference product does.
#![allow(clippy::expect_used, reason = "finite product controls")]

use super::ImplicitProduct;
use crate::Graph;
use crate::formula::{self, Atom, AtomKind, Formula, Node};
use crate::graph::GraphAccess;
use crate::product::{AcceptancePolicy, CompactProduct, Labels, Limits};

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % u64::try_from(n).expect("small")).expect("small")
    }

    fn bit(&mut self) -> bool {
        self.next() & 1 == 1
    }
}

fn atom(kind: AtomKind, index: usize) -> Node {
    Node::Atom(Atom { kind, index })
}

// Formulas over state atoms 0..2 and action atoms 0..2, as negated
// properties reach the product: eventualities, recurrences, persistence,
// conjunctions of per-sender persistence obligations and action atoms.
fn formulas() -> Vec<Formula> {
    let s0 = || atom(AtomKind::State, 0);
    let s1 = || atom(AtomKind::State, 1);
    let a0 = || atom(AtomKind::Action, 0);
    let shapes: Vec<(Vec<Node>, usize)> = vec![
        (vec![Node::Bool(true)], 0),
        (vec![s0(), Node::Always(0), Node::Eventually(1)], 2),
        (
            vec![
                s0(),
                Node::Eventually(0),
                Node::Always(1),
                s1(),
                Node::Not(3),
                Node::Eventually(4),
                Node::Always(5),
                Node::And(vec![2, 6]),
            ],
            7,
        ),
        (
            vec![
                s0(),
                s1(),
                Node::Not(1),
                Node::Eventually(2),
                Node::Always(3),
                Node::And(vec![0, 4]),
                Node::Eventually(5),
            ],
            6,
        ),
        (
            vec![
                s0(),
                s1(),
                Node::Not(1),
                Node::Eventually(2),
                Node::Always(3),
                Node::And(vec![0, 4]),
                Node::Not(0),
                Node::Eventually(6),
                Node::Always(7),
                Node::And(vec![1, 8]),
                Node::And(vec![5, 9]),
                Node::Eventually(10),
            ],
            11,
        ),
        (
            vec![
                a0(),
                Node::Not(0),
                Node::Always(1),
                Node::Eventually(2),
                s1(),
                Node::Eventually(4),
                Node::Always(5),
                Node::And(vec![3, 6]),
            ],
            7,
        ),
        (
            vec![
                s0(),
                atom(AtomKind::Action, 1),
                Node::Eventually(1),
                Node::Or(vec![0, 2]),
                Node::Always(3),
                Node::Not(4),
            ],
            5,
        ),
    ];
    shapes
        .into_iter()
        .map(|(nodes, root)| Formula::new(nodes, root).expect("formula"))
        .collect()
}

fn random_case(rng: &mut Rng) -> (Graph, Labels) {
    let vertices = 1 + rng.below(5);
    let edges: Vec<Vec<usize>> = (0..vertices)
        .map(|_| (0..rng.below(4)).map(|_| rng.below(vertices)).collect())
        .collect();
    let mut roots: Vec<usize> = (0..vertices).filter(|_| rng.below(3) == 0).collect();
    if roots.is_empty() {
        roots.push(rng.below(vertices));
    }
    let states = (0..2)
        .map(|_| (0..vertices).map(|_| rng.bit()).collect())
        .collect();
    let actions = (0..2)
        .map(|_| {
            edges
                .iter()
                .map(|row| row.iter().map(|_| rng.bit()).collect())
                .collect()
        })
        .collect();
    let graph = Graph::new(edges, roots).expect("graph");
    (graph, Labels { states, actions })
}

fn policies() -> [AcceptancePolicy; 3] {
    [
        AcceptancePolicy::Cells,
        AcceptancePolicy::compact_default(),
        AcceptancePolicy::Buffers {
            bytes: usize::MAX,
            work: usize::MAX,
        },
    ]
}

#[test]
fn on_the_fly_products_match_materialized_products_and_witnesses() {
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let formulas = formulas();
    let mut witnesses = 0;
    let mut empty = 0;
    for _ in 0..400 {
        let (graph, labels) = random_case(&mut rng);
        for formula in &formulas {
            let groups =
                formula::decompose(formula, formula::Limits::default()).expect("solution groups");
            for group in groups {
                let tableau = formula::compile_residual(&group.core, formula::Limits::default())
                    .expect("tableau");
                for policy in policies() {
                    let compact = CompactProduct::with_policy(
                        &graph,
                        &tableau,
                        &labels,
                        Limits::default(),
                        policy,
                    )
                    .expect("materialized product");
                    let implicit =
                        ImplicitProduct::new(&graph, &tableau, &labels, Limits::default(), policy)
                            .expect("on-the-fly product");
                    let rows = &compact.parts.graph;
                    let edges = (0..rows.vertex_count())
                        .map(|v| rows.successors(v).len())
                        .sum();
                    assert_eq!(implicit.dimensions(), (rows.vertex_count(), edges));
                    assert_eq!(implicit.roots, rows.roots());
                    assert_eq!(implicit.states, compact.parts.behavior_states);
                    let mut first = Ok(None);
                    for alternative in &group.alternatives {
                        let expected = compact.find_lasso_with_conditions(&labels, alternative);
                        if matches!(first, Ok(None)) {
                            first = expected.clone();
                        }
                        assert_eq!(
                            implicit.find_lasso_with_conditions(&labels, alternative),
                            expected
                        );
                        match expected {
                            Ok(Some(_)) => witnesses += 1,
                            Ok(None) => empty += 1,
                            Err(_) => {}
                        }
                    }
                    assert_eq!(
                        implicit.find_lasso_in_alternatives(&labels, &group.alternatives),
                        first
                    );
                }
            }
        }
    }
    // Both outcomes are exercised, with and without cycle conditions.
    assert!(witnesses > 1000 && empty > 1000, "{witnesses} {empty}");
}

#[test]
fn on_the_fly_products_report_the_same_named_construction_limits() {
    let mut rng = Rng(0x2545_f491_4f6c_dd1d);
    let formulas = formulas();
    let mut refusals = 0;
    for _ in 0..60 {
        let (graph, labels) = random_case(&mut rng);
        for formula in &formulas {
            for group in
                formula::decompose(formula, formula::Limits::default()).expect("solution groups")
            {
                let tableau = formula::compile_residual(&group.core, formula::Limits::default())
                    .expect("tableau");
                for bound in 0..12 {
                    for limits in [
                        Limits {
                            vertices: bound,
                            ..Limits::default()
                        },
                        Limits {
                            edges: bound,
                            ..Limits::default()
                        },
                        Limits {
                            steps: bound * 4,
                            ..Limits::default()
                        },
                        Limits {
                            acceptance_cells: bound * 2,
                            ..Limits::default()
                        },
                    ] {
                        for policy in [
                            AcceptancePolicy::Cells,
                            AcceptancePolicy::Buffers {
                                bytes: bound * 16,
                                work: bound * 4,
                            },
                        ] {
                            let compact = CompactProduct::with_policy(
                                &graph, &tableau, &labels, limits, policy,
                            );
                            let implicit =
                                ImplicitProduct::new(&graph, &tableau, &labels, limits, policy);
                            assert_eq!(implicit.as_ref().err(), compact.as_ref().err());
                            if let (Ok(compact), Ok(implicit)) = (compact, implicit) {
                                for alternative in &group.alternatives {
                                    assert_eq!(
                                        implicit.find_lasso_with_conditions(&labels, alternative),
                                        compact.find_lasso_with_conditions(&labels, alternative)
                                    );
                                }
                            } else {
                                refusals += 1;
                            }
                        }
                    }
                }
            }
        }
    }
    assert!(refusals > 1000, "{refusals}");
}

#[test]
fn an_expired_run_budget_stops_discovery_and_search() {
    let graph = Graph::new(vec![vec![0]], vec![0]).expect("graph");
    let labels = Labels {
        states: vec![vec![true], vec![true]],
        actions: vec![vec![vec![true]], vec![vec![true]]],
    };
    let tableau = formula::compile(&formulas()[1], formula::Limits::default()).expect("tableau");
    let product = ImplicitProduct::new(
        &graph,
        &tableau,
        &labels,
        Limits::default(),
        AcceptancePolicy::Cells,
    )
    .expect("product");
    let watchdog = crate::budget::expired_for_test();
    let _scope = crate::budget::enter(watchdog.budget());
    let none = crate::formula::CycleConditions {
        always: vec![],
        recurring: vec![],
    };
    assert_eq!(
        product.find_lasso_with_conditions(&labels, &none),
        Err(crate::product::ProductError::Limit(
            crate::budget::EXHAUSTED
        ))
    );
    assert_eq!(
        ImplicitProduct::new(
            &graph,
            &tableau,
            &labels,
            Limits::default(),
            AcceptancePolicy::Cells,
        )
        .err(),
        Some(crate::product::ProductError::Limit(
            crate::budget::EXHAUSTED
        ))
    );
}

#[test]
fn repeated_product_rows_keep_parallel_edges_and_honor_the_byte_cap() {
    use crate::search::Adjacency;
    let graph = Graph::new(vec![vec![1], vec![2, 2], vec![1]], vec![0]).expect("graph");
    let labels = Labels {
        states: vec![vec![false; 3]],
        actions: vec![vec![vec![false], vec![false, true], vec![false]]],
    };
    let formula = Formula::new(
        vec![
            atom(AtomKind::State, 0),
            Node::Always(0),
            Node::Eventually(1),
            atom(AtomKind::Action, 0),
            Node::Eventually(3),
            Node::Always(4),
            Node::Or(vec![2, 5]),
        ],
        6,
    )
    .expect("FG state or GF action");
    let groups = formula::decompose(&formula, formula::Limits::default()).expect("groups");
    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert_eq!(group.alternatives.len(), 2);
    let tableau =
        formula::compile_residual(&group.core, formula::Limits::default()).expect("tableau");
    let product = ImplicitProduct::new(
        &graph,
        &tableau,
        &labels,
        Limits::default(),
        AcceptancePolicy::Cells,
    )
    .expect("product");
    let bytes = (product.vertex_count() + 1) * size_of::<u32>()
        + product.edge_count * size_of::<(u32, u32)>();
    assert!(
        super::RepeatedRows::build(&product, bytes - 1)
            .expect("fall back")
            .is_none()
    );
    let rows = super::RepeatedRows::build(&product, bytes)
        .expect("build")
        .expect("fits");
    let compare = |path: &crate::search::Lasso| {
        assert!(
            path.cycle
                .iter()
                .any(|step| step.from == 1 && step.edge == 1 && step.to == 2)
        );
    };
    let original = product
        .search_rows(&product, &labels, &group.alternatives)
        .expect("implicit")
        .expect("lasso");
    compare(&original);
    assert_eq!(
        product
            .search_rows(&rows, &labels, &group.alternatives)
            .expect("stored")
            .as_ref(),
        Some(&original)
    );
}
