//! Finite admission and allocation controls, independent of corpus models.
#![allow(clippy::expect_used, clippy::panic, reason = "finite test assertions")]

use super::*;
use crate::Graph;
use crate::formula::{Atom, AtomKind, CycleConditions, Formula, Node, Recurrence, TableauNode};
use crate::product::{CompactProduct, ImplicitProduct, Labels, Product};
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
    .expect("formula");
    // The body of `[]<>A`, as solution decomposition extracts it.
    vec![Recurrence::condition(&formula, &[0]).expect("recurrence")]
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
fn word_boundaries_exact_bytes_work_truth_and_witness_match_owned() {
    for vertices in [0, 1, 63, 64, 65, 127, 128, 129] {
        for promises in [0, 1, 3] {
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
            let layout = Layout::new(vertices, promises).expect("layout");
            // One unit per promise assignment, header and zeroed word.
            let work = vertices * promises + promises + promises * vertices.div_ceil(64);
            let owned = Product::new(&graph, &t, &labels, Limits::default()).expect("owned");
            let compact = CompactProduct::with_policy(
                &graph,
                &t,
                &labels,
                Limits::default(),
                policy(layout.bytes, work),
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
                }
            }
            assert_eq!(compact.find_lasso(), owned.find_lasso());
            assert_eq!(compact.parts.acceptance.materialization_work, work);
            // Capacity accounting includes the outer header buffer.
            assert_eq!(covered_bytes(&compact, 0), layout.bytes);
            let implicit = |bytes, work| {
                ImplicitProduct::new(&graph, &t, &labels, Limits::default(), policy(bytes, work))
                    .err()
            };
            assert_eq!(implicit(layout.bytes, work), None);
            if layout.bytes > 0 {
                assert_eq!(
                    refusal(CompactProduct::with_policy(
                        &graph,
                        &t,
                        &labels,
                        Limits::default(),
                        policy(layout.bytes - 1, work)
                    )),
                    BYTES
                );
                assert_eq!(implicit(layout.bytes - 1, work), Some(BYTES));
            }
            if work > 0 {
                assert_eq!(
                    refusal(CompactProduct::with_policy(
                        &graph,
                        &t,
                        &labels,
                        Limits::default(),
                        policy(layout.bytes, work - 1)
                    )),
                    WORK
                );
                assert_eq!(implicit(layout.bytes, work - 1), Some(WORK));
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
        refusal(CompactProduct::new(&graph, &t, &labels, limits)),
        ProductError::Limit("acceptance cells")
    );
    let admitted = CompactProduct::with_policy(
        &graph,
        &t,
        &labels,
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
            Limits::default(),
            policy(100, 0)
        )),
        WORK
    );
}

#[test]
fn checked_layout_overflow_and_fallible_allocation_mapping() {
    for (vertices, promises) in [(1, usize::MAX), (usize::MAX, 64)] {
        assert!(matches!(
            Layout::new(vertices, promises),
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
    a.states.capacity() * size_of::<Vec<u64>>()
        + a.states
            .iter()
            .map(|row| row.capacity() * size_of::<u64>())
            .sum::<usize>()
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
        let t = tableau(promises);
        let probe =
            CompactProduct::with_policy(&graph, &t, &labels, Limits::default(), policy(1000, 1000))
                .expect("probe actual setup");
        // Each promise: header, one word and one assignment.
        let base_work = 3 * promises;
        assert_eq!(probe.parts.acceptance.materialization_work, base_work);
        // Two inspected solution conditions and two initialized scratch
        // entries. Searching their truth does not spend materialization.
        let needed_work = base_work + 4;
        let promise_bytes = size_of::<Vec<u64>>() + size_of::<u64>();
        let needed_bytes = promise_bytes * promises + 2;
        assert_eq!(covered_bytes(&probe, 2), needed_bytes);
        let compact = |bytes, work| {
            CompactProduct::with_policy(&graph, &t, &labels, Limits::default(), policy(bytes, work))
                .and_then(|p| p.find_lasso_with_conditions(&labels, &solution))
        };
        let implicit = |bytes, work| {
            ImplicitProduct::new(&graph, &t, &labels, Limits::default(), policy(bytes, work))
                .and_then(|p| p.find_lasso_with_conditions(&labels, &solution))
        };
        for search in [&compact as &dyn Fn(usize, usize) -> _, &implicit] {
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
    // One promise column and the two-entry scratch of one alternative.
    let bytes = size_of::<Vec<u64>>() + size_of::<u64>() + 2;
    // The residual core spends 3 actual setup units. Each alternative then
    // inspects 2 conditions and initializes 2 scratch cells.
    let budget = policy(bytes, 3 + 2 * 4);
    let t = tableau(1);
    let compact = CompactProduct::with_policy(&graph, &t, &labels, Limits::default(), budget)
        .expect("shared base");
    let implicit =
        ImplicitProduct::new(&graph, &t, &labels, Limits::default(), budget).expect("shared base");
    let searches: [&dyn Fn() -> _; 2] = [
        &|| compact.find_lasso_with_conditions(&labels, &solution),
        &|| implicit.find_lasso_with_conditions(&labels, &solution),
    ];
    for search in searches {
        for _ in 0..2 {
            assert_eq!(
                search(),
                Ok(None),
                "sequential scratch fits the same live-byte ceiling"
            );
        }
        assert_eq!(
            search(),
            Err(WORK),
            "a third alternative cannot reuse spent materialization work"
        );
    }
}

#[test]
fn all_small_complete_tableaux_match_independent_language_and_full_witness() {
    use crate::search::{Lasso, Step};
    let graph = Graph::new(vec![vec![0, 0]], vec![0]).expect("graph");
    // `[]<>A` is the solution's recurring cycle condition.
    let solution = CycleConditions {
        always: vec![],
        recurring: conditions(),
    };
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
                        Limits::default(),
                        AcceptancePolicy::compact_default(),
                    )
                    .expect("finite");
                    for q in 0..nodes {
                        assert_eq!(compact.state(0, q), first & (1 << q) != 0);
                        assert_eq!(compact.state(1, q), second & (1 << q) != 0);
                    }
                    // All tableau nodes mutually reach each other. Each promise
                    // needs a marked node and the recurrent walk needs a true edge.
                    let language = first != 0 && second != 0 && action != 0;
                    let word = compact
                        .find_lasso_with_conditions(&labels, &solution)
                        .expect("search");
                    assert_eq!(word.is_some(), language);
                    for policy in [AcceptancePolicy::Cells, AcceptancePolicy::compact_default()] {
                        assert_eq!(
                            ImplicitProduct::new(&graph, &t, &labels, Limits::default(), policy)
                                .expect("on-the-fly product")
                                .find_lasso_with_conditions(&labels, &solution)
                                .expect("on-the-fly search"),
                            word
                        );
                    }
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

#[test]
fn actual_budget_stops_acceptance_materialization_without_a_count_ceiling() {
    let pairs = [(0, 0)];
    let order = [0];
    let input = Ordered {
        pairs: &pairs,
        order: &order,
    };
    let t = tableau(1);
    let run = || materialize(&input, &t, usize::MAX, usize::MAX);
    {
        let watchdog = crate::budget::expired_for_test();
        let _scope = crate::budget::enter(watchdog.budget());
        assert!(matches!(
            run(),
            Err(ProductError::Limit(crate::budget::EXHAUSTED))
        ));
    }
    assert!(
        run().is_ok(),
        "the restored owner can materialize exact promise bits"
    );
}
