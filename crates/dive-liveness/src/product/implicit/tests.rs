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
        .search_rows(&product, &labels, &group.alternatives, 0)
        .expect("implicit")
        .expect("lasso");
    compare(&original);
    assert_eq!(
        product
            .search_rows(&rows, &labels, &group.alternatives, 0)
            .expect("stored")
            .as_ref(),
        Some(&original)
    );
}

#[test]
fn grouped_fairness_elimination_matches_ordinary_search_on_labelled_multigraphs() {
    // Three independent choices of FG or GF, including cases with
    // parallel edges whose truth differs even though their targets agree.
    let mut nodes = Vec::new();
    for index in 0..3 {
        let n = nodes.len();
        nodes.extend([
            atom(AtomKind::State, index % 2),
            Node::Always(n),
            Node::Eventually(n + 1),
            atom(AtomKind::Action, usize::from(index > 0)),
            Node::Eventually(n + 3),
            Node::Always(n + 4),
            Node::Or(vec![n + 2, n + 5]),
        ]);
    }
    nodes.push(Node::And(vec![6, 13, 20]));
    let formula = Formula::new(nodes, 21).expect("formula");
    let groups = formula::decompose(&formula, formula::Limits::default()).expect("groups");
    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert_eq!(group.alternatives.len(), 8);
    let tableau =
        formula::compile_residual(&group.core, formula::Limits::default()).expect("tableau");
    let formula::FactoredSolutions {
        groups: factored,
        pairs,
    } = formula::streett(&formula, formula::Limits::default()).expect("factored");
    assert_eq!(factored.len(), 1);
    assert_eq!(factored[0].alternatives.len(), 1);
    assert_eq!(pairs.len(), 3);
    let mut rng = Rng(0x7771);
    let mut witnesses = 0;
    let mut eliminated = 0;
    for _ in 0..100 {
        let (graph, labels) = random_case(&mut rng);
        let mut targets = Vec::new();
        let mut offsets = vec![0];
        for row in &graph.edges {
            targets.extend(
                row.iter()
                    .map(|&target| u32::try_from(target).expect("small graph")),
            );
            offsets.push(targets.len());
        }
        let topology =
            crate::product::CsrTopology::new(&offsets, &targets, &graph.roots).expect("csr");
        let product = ImplicitProduct::new(
            &topology,
            &tableau,
            &labels,
            Limits::default(),
            AcceptancePolicy::Cells,
        )
        .expect("product");
        let reference = product
            .search_rows(&product, &labels, &group.alternatives, 0)
            .expect("ordinary search");
        let lazy = product
            .find_lasso_with_streett(&labels, &factored[0].alternatives, &pairs, None)
            .expect("factored search");
        assert_eq!(lazy.is_some(), reference.is_some());
        if let Some(lasso) = lazy {
            for index in 0..3 {
                assert!(
                    lasso.cycle.iter().all(|s| labels.states[index % 2][s.from])
                        || lasso
                            .cycle
                            .iter()
                            .any(|s| labels.actions[usize::from(index > 0)][s.from][s.edge])
                );
            }
        }
        let mut bytes = usize::MAX;
        let words =
            super::ConditionWords::build(&offsets, &group.alternatives, &labels, &mut bytes)
                .expect("index")
                .expect("eligible");
        if words
            .rules_out(&product, &product, bytes)
            .expect("elimination")
        {
            assert!(reference.is_none());
            eliminated += 1;
        }
        witnesses += usize::from(reference.is_some());
        assert_eq!(
            product
                .find_lasso_in_alternatives(&labels, &group.alternatives)
                .expect("grouped search"),
            reference
        );
    }
    assert!(witnesses > 0 && eliminated > 0);
}

fn streett_test_formula() -> Formula {
    let mut nodes = vec![
        atom(AtomKind::State, 0),
        atom(AtomKind::Action, 0),
        atom(AtomKind::State, 1),
        atom(AtomKind::Action, 1),
        Node::Always(0),
        Node::Eventually(4),
        Node::Eventually(1),
        Node::Always(6),
        Node::Or(vec![5, 7]),
        Node::Always(3),
        Node::Eventually(9),
        Node::Eventually(2),
        Node::Always(11),
        Node::Or(vec![10, 12]),
        Node::Or(vec![0, 3]),
        Node::Eventually(14),
        Node::Always(15),
        Node::Not(1),
        Node::Always(17),
        Node::Eventually(18),
        Node::Always(2),
        Node::And(vec![0, 20]),
        Node::Eventually(21),
    ];
    let root = nodes.len();
    nodes.push(Node::And(vec![8, 13, 16, 19, 22]));
    Formula::new(nodes, root).expect("two Streett pairs, base GF/FG, residual promise")
}

#[test]
fn streett_components_match_exhaustive_alternatives_and_original_word_truth() {
    let f = streett_test_formula();
    let explicit = formula::decompose(&f, formula::Limits::default()).expect("explicit");
    let factored = formula::streett(&f, formula::Limits::default()).expect("factored");
    let mut rng = Rng(0x1311);
    let mut cases = [0; 2];
    for _ in 0..500 {
        let (graph, labels) = random_case(&mut rng);
        let mut targets = Vec::new();
        let mut offsets = vec![0];
        for row in &graph.edges {
            targets.extend(row.iter().map(|&n| u32::try_from(n).expect("small")));
            offsets.push(targets.len());
        }
        let csr = crate::product::CsrTopology::new(&offsets, &targets, &graph.roots).expect("csr");
        let words = super::StreettWords::new(
            (&csr).into(),
            &labels,
            &factored.pairs,
            factored.groups.iter().flat_map(|g| &g.alternatives),
        )
        .expect("index")
        .expect("fits");
        let mut expected = false;
        for group in &explicit {
            let tab = formula::compile_residual(&group.core, formula::Limits::default())
                .expect("tableau");
            let model = CompactProduct::with_policy(
                &graph,
                &tab,
                &labels,
                Limits::default(),
                AcceptancePolicy::Cells,
            )
            .expect("reference");
            for alt in &group.alternatives {
                expected |= model
                    .find_lasso_with_conditions(&labels, alt)
                    .expect("reference word")
                    .is_some();
            }
        }
        let mut found = false;
        for group in &factored.groups {
            let tab = formula::compile_residual(&group.core, formula::Limits::default())
                .expect("tableau");
            let model = ImplicitProduct::new(
                &csr,
                &tab,
                &labels,
                Limits::default(),
                AcceptancePolicy::Cells,
            )
            .expect("factored");
            assert!(
                matches!(
                    words
                        .solve(&model, &model, &group.alternatives[0], 0)
                        .expect("optional cap"),
                    super::streett_components::ComponentVerdict::Unknown
                ) || model.states.is_empty()
            );
            if let Some(word) = model
                .find_lasso_with_streett(
                    &labels,
                    &group.alternatives,
                    &factored.pairs,
                    Some(&words),
                )
                .expect("factored word")
            {
                let mut states = Vec::new();
                let mut actions = Vec::new();
                for step in word.prefix.iter().chain(&word.cycle) {
                    assert_eq!(graph.edges[step.from][step.edge], step.to);
                    states.push(labels.states.iter().map(|atom| atom[step.from]).collect());
                    actions.push(
                        labels
                            .actions
                            .iter()
                            .map(|atom| atom[step.from][step.edge])
                            .collect(),
                    );
                }
                assert!(
                    f.evaluate_word(&states, &actions, word.prefix.len(), usize::MAX)
                        .expect("original word semantics")[0]
                );
                found = true;
            }
        }
        assert_eq!(found, expected);
        cases[usize::from(found)] += 1;
    }
    assert!(cases.iter().all(|n| *n > 0));
}

#[test]
fn an_absent_grant_can_force_a_second_persistence_filter() {
    use super::streett_components::ComponentVerdict;
    let f = Formula::new(
        vec![
            atom(AtomKind::State, 0),
            Node::Always(0),
            Node::Eventually(1),
            atom(AtomKind::Action, 0),
            Node::Eventually(3),
            Node::Always(4),
            Node::Or(vec![2, 5]),
            atom(AtomKind::State, 1),
            Node::Always(7),
            Node::Eventually(8),
            atom(AtomKind::Action, 1),
            Node::Eventually(10),
            Node::Always(11),
            Node::Or(vec![9, 12]),
            Node::And(vec![6, 13]),
        ],
        14,
    )
    .expect("two pairs");
    let plan = formula::streett(&f, formula::Limits::default()).expect("factored");
    let group = &plan.groups[0];
    let tableau =
        formula::compile_residual(&group.core, formula::Limits::default()).expect("tableau");
    for has_loop in [false, true] {
        // Removing 0's outgoing edge removes the only reachable second
        // grant. Vertex 2 must then persist forever. Vertex 3 is unreachable.
        let edges = vec![
            vec![1],
            vec![0, 2],
            if has_loop { vec![1, 2] } else { vec![1] },
            vec![3],
        ];
        let labels = Labels {
            states: vec![
                vec![false, true, true, true],
                vec![false, false, true, true],
            ],
            actions: vec![
                edges.iter().map(|r| vec![false; r.len()]).collect(),
                edges
                    .iter()
                    .enumerate()
                    .map(|(v, r)| vec![v == 0; r.len()])
                    .collect(),
            ],
        };
        let mut offsets = vec![0];
        let mut targets = Vec::new();
        for row in edges {
            targets.extend(row);
            offsets.push(targets.len());
        }
        let csr = crate::product::CsrTopology::new(&offsets, &targets, &[0]).expect("csr");
        let product = ImplicitProduct::new(
            &csr,
            &tableau,
            &labels,
            Limits::default(),
            AcceptancePolicy::Cells,
        )
        .expect("product");
        let words = super::StreettWords::new(
            (&csr).into(),
            &labels,
            &plan.pairs,
            group.alternatives.iter(),
        )
        .expect("words")
        .expect("fits");
        let verdict = words
            .solve(&product, &product, &group.alternatives[0], usize::MAX)
            .expect("solve");
        assert!(matches!(
            (has_loop, verdict),
            (true, ComponentVerdict::Accept(0)) | (false, ComponentVerdict::Empty)
        ));
    }
}
