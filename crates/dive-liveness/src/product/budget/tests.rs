//! Finite admission and allocation controls, independent of corpus models.
#![allow(clippy::expect_used, clippy::panic, reason = "finite test assertions")]

use super::*;
use crate::Graph;
use crate::formula::{Atom, AtomKind, CycleConditions, Formula, Node, TableauNode};
use crate::product::{CompactProduct, Labels, Product};
use crate::search::AcceptanceAccess;

fn tableau(promises: usize) -> Tableau {
    Tableau {
        nodes: vec![TableauNode {
            literals: vec![],
            successors: vec![0],
            fulfilled: vec![true; promises],
        }],
        roots: vec![0],
        promise_count: promises,
    }
}

fn conditions() -> Vec<Recurrence> {
    Formula::new(
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
    .expect("formula")
    .factor_recurrences(100)
    .expect("factor")
    .1
}

fn policy(bytes: usize, work: usize) -> AcceptancePolicy {
    AcceptancePolicy::Buffers { bytes, work }
}

fn refusal(result: Result<CompactProduct, ProductError>) -> ProductError {
    match result {
        Err(error) => error,
        Ok(_) => panic!("expected refusal"),
    }
}

#[test]
fn word_boundaries_exact_bytes_truth_and_witness_match_owned() {
    let conditions = conditions();
    for vertices in [0, 1, 63, 64, 65, 127, 128, 129] {
        for promises in [0, 1, 3] {
            for recurrence in [false, true] {
                let graph = Graph::new(
                    (0..vertices)
                        .map(|v| {
                            if v % 7 == 0 {
                                vec![]
                            } else {
                                vec![v, (v + 1) % vertices, v]
                            }
                        })
                        .collect(),
                    (0..vertices).collect(),
                )
                .expect("graph");
                let t = tableau(promises);
                let labels = Labels {
                    states: vec![],
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
                let cs = if recurrence { &conditions[..] } else { &[] };
                let edges = graph.edges.iter().map(Vec::len).sum();
                let layout = Layout::new(
                    vertices,
                    edges,
                    promises,
                    cs.len(),
                    if recurrence { 2 } else { 0 },
                )
                .expect("layout");
                let owned = Product::with_recurrences(&graph, &t, &labels, cs, Limits::default())
                    .expect("owned");
                let compact = CompactProduct::with_policy(
                    &graph,
                    &t,
                    &labels,
                    cs,
                    Limits::default(),
                    policy(layout.bytes, 100_000),
                )
                .expect("exact budget");
                assert_eq!(compact.parts.graph.owned_graph(), owned.graph);
                assert_eq!(
                    compact.parts.graph.owned_original_edges(),
                    owned.original_edges
                );
                for p in 0..promises {
                    for v in 0..vertices {
                        assert_eq!(compact.state(p, v), owned.acceptance.states[p][v]);
                    }
                }
                for (v, row) in owned.graph.edges.iter().enumerate() {
                    for e in 0..row.len() {
                        assert!(compact.recurrent(v, e));
                        if recurrence {
                            assert_eq!(compact.action(0, v, e), labels.actions[0][v][e]);
                        }
                    }
                }
                assert_eq!(compact.find_lasso(), owned.find_lasso());
                // Capacity accounting includes both outer header buffers and scratch.
                let a = &compact.parts.acceptance;
                let actual = (a.states.capacity() + a.actions.capacity()) * size_of::<Vec<u64>>()
                    + a.states
                        .iter()
                        .chain(&a.actions)
                        .map(|r| r.capacity() * size_of::<u64>())
                        .sum::<usize>()
                    + a.edge_offsets.capacity() * size_of::<usize>()
                    + layout.scratch;
                assert_eq!(actual, layout.bytes);
                if layout.bytes > 0 {
                    assert_eq!(
                        refusal(CompactProduct::with_policy(
                            &graph,
                            &t,
                            &labels,
                            cs,
                            Limits::default(),
                            policy(layout.bytes - 1, 100_000)
                        )),
                        BYTES
                    );
                }
            }
        }
    }
}

#[test]
fn byte_mode_replaces_cells_but_preserves_other_guards_and_legacy_priority() {
    let graph = Graph::new(vec![vec![0, 0]], vec![0]).expect("graph");
    let labels = Labels {
        states: vec![],
        actions: vec![],
    };
    let t = tableau(1);
    let limits = Limits {
        acceptance_cells: 0,
        ..Limits::default()
    };
    assert_eq!(
        Product::new(&graph, &t, &labels, limits).expect_err("legacy"),
        ProductError::Limit("acceptance cells")
    );
    assert_eq!(
        refusal(CompactProduct::with_recurrences(
            &graph,
            &t,
            &labels,
            &[],
            limits
        )),
        ProductError::Limit("acceptance cells")
    );
    let admitted = CompactProduct::with_policy(
        &graph,
        &t,
        &labels,
        &[],
        limits,
        policy(size_of::<Vec<u64>>() + size_of::<u64>(), 3),
    )
    .expect("bytes fit");
    assert!(admitted.find_lasso().expect("search").is_some());
    for (limits, expected) in [
        (
            Limits {
                vertices: 0,
                ..Limits::default()
            },
            "vertices",
        ),
        (
            Limits {
                edges: 0,
                ..Limits::default()
            },
            "edges",
        ),
        (
            Limits {
                steps: 0,
                ..Limits::default()
            },
            "transition steps",
        ),
    ] {
        assert_eq!(
            refusal(CompactProduct::with_policy(
                &graph,
                &t,
                &labels,
                &[],
                limits,
                policy(100, 100)
            )),
            ProductError::Limit(expected)
        );
    }
    let mut malformed = t;
    malformed.roots = vec![1];
    assert!(matches!(
        refusal(CompactProduct::with_policy(
            &graph,
            &malformed,
            &labels,
            &[],
            limits,
            policy(0, 0)
        )),
        ProductError::Shape(_)
    ));
}

#[test]
fn zero_budget_empty_columns_and_independent_work_bound() {
    let graph = Graph::new(vec![vec![0]], vec![0]).expect("graph");
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![true]]],
    };
    assert!(
        CompactProduct::with_policy(
            &graph,
            &tableau(0),
            &labels,
            &[],
            Limits::default(),
            policy(0, 0)
        )
        .is_ok()
    );
    assert_eq!(
        refusal(CompactProduct::with_policy(
            &graph,
            &tableau(1),
            &labels,
            &[],
            Limits::default(),
            policy(100, 0)
        )),
        WORK
    );
    let cs = conditions();
    // 4 metadata inspections + 2 offsets + 1 header + 1 zeroed word +
    // 2 scratch entries + 1 row + 1 projection + 1 write + 2 nodes + 1 child = 16.
    assert!(
        CompactProduct::with_policy(
            &graph,
            &tableau(0),
            &labels,
            &cs,
            Limits::default(),
            policy(100, 16)
        )
        .is_ok()
    );
    assert_eq!(
        refusal(CompactProduct::with_policy(
            &graph,
            &tableau(0),
            &labels,
            &cs,
            Limits::default(),
            policy(100, 15)
        )),
        WORK
    );
    let no_edges = Graph::new(vec![vec![]], vec![0]).expect("dead end");
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![]]],
    };
    let layout = Layout::new(1, 0, 0, 1, usize::MAX).expect("no scratch without edges");
    assert_eq!(layout.scratch, 0);
    assert!(
        CompactProduct::with_policy(
            &no_edges,
            &tableau(0),
            &labels,
            &cs,
            Limits::default(),
            policy(layout.bytes, 5)
        )
        .is_ok()
    );
}

#[test]
fn checked_layout_overflow_and_fallible_allocation_mapping() {
    for dims in [
        (usize::MAX, 0, 0, 1, 0),
        (1, 1, usize::MAX, 1, 1),
        (1, usize::MAX, 0, usize::MAX, 1),
    ] {
        assert!(matches!(
            Layout::new(dims.0, dims.1, dims.2, dims.3, dims.4),
            Err(ProductError::Limit("acceptance buffer bytes"))
        ));
    }
    let mut requests = Requests {
        used: 0,
        limit: usize::MAX,
    };
    assert_eq!(
        requests
            .reserve::<u8>(usize::MAX)
            .expect_err("capacity overflow"),
        ALLOCATION
    );
    let mut requests = Requests { used: 0, limit: 0 };
    assert_eq!(
        requests
            .reserve::<u64>(1)
            .expect_err("preallocation refusal"),
        BYTES
    );
}

fn covered_bytes(product: &CompactProduct, scratch: usize) -> usize {
    let a = &product.parts.acceptance;
    (a.states.capacity() + a.actions.capacity()) * size_of::<Vec<u64>>()
        + a.states
            .iter()
            .chain(&a.actions)
            .map(|row| row.capacity() * size_of::<u64>())
            .sum::<usize>()
        + a.edge_offsets.capacity() * size_of::<usize>()
        + scratch
}

#[test]
fn solution_setup_shares_the_base_buffers_and_materialization_work_budget() {
    let graph = Graph::new(vec![vec![0, 0]], vec![0]).expect("graph");
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![true, true]]],
    };
    let cs = conditions();
    assert_eq!(cs[0].scratch_len(), 2);
    let solution = CycleConditions {
        always: cs.clone(),
        recurring: cs.clone(),
    };
    for promises in [0, 1] {
        for base_conditions in [false, true] {
            let t = tableau(promises);
            let base = if base_conditions { &cs[..] } else { &[] };
            let probe = CompactProduct::with_policy(
                &graph,
                &t,
                &labels,
                base,
                Limits::default(),
                policy(1000, 1000),
            )
            .expect("probe actual setup");
            // Each promise: header, one word and one assignment. An existing
            // condition: four metadata inspections, eleven initialized/row/
            // projection/write entries, and six executed Boolean operations.
            let base_work = 3 * promises + usize::from(base_conditions) * 21;
            assert_eq!(probe.parts.acceptance.materialization_work, base_work);
            // Two inspected solution conditions and two initialized scratch
            // entries. Searching their truth does not spend materialization.
            let needed_work = base_work + 4;
            let promise_bytes = size_of::<Vec<u64>>() + size_of::<u64>();
            let condition_bytes = promise_bytes + 2 * size_of::<usize>();
            let needed_bytes =
                promise_bytes * promises + usize::from(base_conditions) * condition_bytes + 2;
            assert_eq!(covered_bytes(&probe, 2), needed_bytes);
            let search = |bytes, work| {
                CompactProduct::with_policy(
                    &graph,
                    &t,
                    &labels,
                    base,
                    Limits::default(),
                    policy(bytes, work),
                )
                .and_then(|p| p.find_lasso_with_conditions(&labels, &solution))
            };
            for bytes in [0, needed_bytes - 1] {
                assert_eq!(search(bytes, needed_work), Err(BYTES));
            }
            for work in [0, needed_work - 1] {
                assert_eq!(search(needed_bytes, work), Err(WORK));
            }
            assert_eq!(search(0, 0), Err(BYTES), "bytes retain priority");
            assert!(
                search(needed_bytes, needed_work)
                    .expect("exact setup")
                    .is_some()
            );
        }
    }
}

#[test]
fn sequential_alternatives_share_bytes_but_accumulate_actual_setup_work() {
    let graph = Graph::new(vec![vec![0, 0]], vec![0]).expect("graph");
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![false, false]]],
    };
    let cs = conditions();
    let solution = CycleConditions {
        always: cs.clone(),
        recurring: cs.clone(),
    };
    let bytes = 2 * (size_of::<Vec<u64>>() + size_of::<u64>()) + 2 * size_of::<usize>() + 2;
    // The residual core and existing recurrence spend 24 actual setup units.
    // Each alternative then inspects 2 conditions and initializes 2 scratch cells.
    let product = CompactProduct::with_policy(
        &graph,
        &tableau(1),
        &labels,
        &cs,
        Limits::default(),
        policy(bytes, 24 + 2 * 4),
    )
    .expect("shared base");
    for _ in 0..2 {
        assert_eq!(
            product.find_lasso_with_conditions(&labels, &solution),
            Ok(None),
            "sequential scratch fits the same live-byte ceiling"
        );
    }
    assert_eq!(
        product.find_lasso_with_conditions(&labels, &solution),
        Err(WORK),
        "a third alternative cannot reuse spent materialization work"
    );
}

#[test]
fn behavior_edge_truth_is_reused_without_merging_parallel_positions_or_properties() {
    let graph = Graph::new(vec![vec![0, 0], vec![1, 1]], vec![0, 1]).expect("graph");
    let t = Tableau {
        nodes: (0..4)
            .map(|q| TableauNode {
                literals: vec![],
                successors: vec![0, 1, 2, 3],
                fulfilled: vec![q % 2 == 0, q % 2 == 1],
            })
            .collect(),
        roots: vec![0],
        promise_count: 2,
    };
    let cs = conditions();
    for satisfiable in [true, false] {
        let labels = Labels {
            states: vec![],
            actions: vec![vec![vec![false, satisfiable]; 2]],
        };
        // Independently: 3 headers*24 + 2 promise words*8 + 1 recurrence word*8
        // + 3 offsets*usize + 2 Boolean scratch entries.
        let bytes = 3 * size_of::<Vec<u64>>() + 3 * size_of::<u64>() + 3 * size_of::<usize>() + 2;
        let compact = CompactProduct::with_policy(
            &graph,
            &t,
            &labels,
            &cs,
            Limits::default(),
            policy(bytes, 53),
        )
        .expect("reused domain fits");
        assert!(compact.parts.acceptance.behavior_actions);
        assert_eq!(compact.parts.graph.owned_graph().edges.len(), 8);
        assert_eq!(
            compact
                .parts
                .graph
                .owned_graph()
                .edges
                .iter()
                .map(Vec::len)
                .sum::<usize>(),
            64
        );
        assert_eq!(compact.parts.acceptance.edge_offsets, vec![0, 2, 4]);
        assert_eq!(
            compact.parts.graph.owned_graph().edges[0][0],
            compact.parts.graph.owned_graph().edges[0][4]
        );
        assert!(!compact.action(0, 0, 0));
        assert_eq!(compact.action(0, 0, 4), satisfiable);
        let owned = Product::with_recurrences(&graph, &t, &labels, &cs, Limits::default())
            .expect("owned reference");
        for (v, row) in owned.graph.edges.iter().enumerate() {
            for e in 0..row.len() {
                let behavior = owned.behavior_states[v];
                let original = owned.original_edges[v][e];
                assert_eq!(
                    compact.action(0, v, e),
                    labels.actions[0][behavior][original]
                );
            }
            for promise in 0..2 {
                assert_eq!(
                    compact.state(promise, v),
                    owned.acceptance.states[promise][v]
                );
            }
        }
        assert_eq!(compact.find_lasso(), owned.find_lasso());
        assert_eq!(compact.find_lasso().expect("search").is_some(), satisfiable);
        let covered = covered_bytes(&compact, 2);
        assert_eq!(covered, bytes);
        assert_eq!(
            refusal(CompactProduct::with_policy(
                &graph,
                &t,
                &labels,
                &cs,
                Limits::default(),
                policy(bytes - 1, 1000)
            )),
            BYTES
        );
    }
    let labels = Labels {
        states: vec![],
        actions: vec![vec![vec![false, true]; 2]],
    };
    assert_eq!(
        refusal(CompactProduct::with_policy(
            &graph,
            &t,
            &labels,
            &cs,
            Limits::default(),
            policy(1000, 52)
        )),
        WORK
    );
}

#[test]
fn sparse_unreachable_behavior_domain_keeps_product_fallback() {
    let graph = Graph::new((0..1000).map(|v| vec![v]).collect(), vec![0]).expect("graph");
    let labels = Labels {
        states: vec![],
        actions: vec![(0..1000).map(|v| vec![v == 0]).collect()],
    };
    let cs = conditions();
    let bytes = size_of::<Vec<u64>>() + size_of::<u64>() + 2 * size_of::<usize>() + 2;
    for allowance in [bytes, 20_000] {
        let compact = CompactProduct::with_policy(
            &graph,
            &tableau(0),
            &labels,
            &cs,
            Limits::default(),
            policy(allowance, 16),
        )
        .expect("small product stays admitted");
        assert!(!compact.parts.acceptance.behavior_actions);
        assert_eq!(compact.parts.acceptance.edge_offsets, vec![0, 1]);
        assert_eq!(
            compact.find_lasso(),
            Product::with_recurrences(&graph, &tableau(0), &labels, &cs, Limits::default())
                .expect("owned")
                .find_lasso()
        );
        assert!(compact.action(0, 0, 0));
    }
}

#[test]
fn mode_costs_count_initialization_lookup_and_actual_boolean_operations() {
    let cs = conditions();
    let programs = Programs::inspect(&cs, 64, 1000).expect("metadata");
    assert_eq!(
        (
            programs.nodes,
            programs.references,
            programs.inspected,
            programs.scratch
        ),
        (2, 1, 4, 2)
    );
    let product = Plan::new(
        Layout::new(8, 64, 2, 1, 2).expect("layout"),
        8,
        2,
        1,
        &programs,
    )
    .expect("cost");
    let behavior = Plan::new(
        Layout::with_domain(8, 2, 1, 2, ActionDomain::behavior(2, 4)).expect("layout"),
        8,
        2,
        1,
        &programs,
    )
    .expect("cost");
    // Product base: metadata4 + promises16 + offsets9 + headers3 +
    // initialized state/action words3 + scratch2 + rows8 + projections/writes128.
    assert_eq!(
        (product.base, product.minimum, product.maximum),
        (173, 301, Some(365))
    );
    // Behavior base: metadata4 + promises16 + offsets3 + headers3 +
    // initialized words3 + scratch2 + rows2 + projections/writes8.
    assert_eq!(
        (behavior.base, behavior.minimum, behavior.maximum),
        (41, 49, Some(53))
    );
    assert!(Programs::inspect(&cs, 1, 3).is_err());
}

#[test]
fn all_small_complete_tableaux_match_independent_language_and_full_witness() {
    use crate::search::{Lasso, Step};
    let graph = Graph::new(vec![vec![0, 0]], vec![0]).expect("graph");
    let cs = conditions();
    for nodes in 1..=3 {
        for first in 0..(1 << nodes) {
            for second in 0..(1 << nodes) {
                let t = Tableau {
                    nodes: (0..nodes)
                        .map(|q| TableauNode {
                            literals: vec![],
                            successors: (0..nodes).collect(),
                            fulfilled: vec![first & (1 << q) != 0, second & (1 << q) != 0],
                        })
                        .collect(),
                    roots: vec![0],
                    promise_count: 2,
                };
                for action in 0..4 {
                    let labels = Labels {
                        states: vec![],
                        actions: vec![vec![vec![action & 1 != 0, action & 2 != 0]]],
                    };
                    let compact = CompactProduct::with_policy(
                        &graph,
                        &t,
                        &labels,
                        &cs,
                        Limits::default(),
                        AcceptancePolicy::compact_default(),
                    )
                    .expect("finite");
                    for q in 0..nodes {
                        assert_eq!(compact.state(0, q), first & (1 << q) != 0);
                        assert_eq!(compact.state(1, q), second & (1 << q) != 0);
                        for position in 0..(2 * nodes) {
                            assert_eq!(
                                compact.action(0, q, position),
                                action & (1 << (position / nodes)) != 0
                            );
                        }
                    }
                    // All tableau nodes mutually reach each other. Each promise
                    // needs a marked node and the recurrent walk needs a true edge.
                    let language = first != 0 && second != 0 && action != 0;
                    let word = compact.find_lasso().expect("search");
                    assert_eq!(word.is_some(), language);
                    assert_eq!(
                        word,
                        Product::with_recurrences(&graph, &t, &labels, &cs, Limits::default())
                            .expect("owned")
                            .find_lasso()
                            .expect("owned search")
                    );
                    if nodes == 2 && first == 1 && second == 2 && action == 2 {
                        assert_eq!(
                            word,
                            Some(Lasso {
                                root: 0,
                                entry: 0,
                                prefix: vec![],
                                cycle: vec![
                                    Step {
                                        from: 0,
                                        edge: 1,
                                        to: 0
                                    },
                                    Step {
                                        from: 0,
                                        edge: 0,
                                        to: 0
                                    },
                                    Step {
                                        from: 0,
                                        edge: 0,
                                        to: 0
                                    },
                                ]
                            })
                        );
                    }
                }
            }
        }
    }
}
