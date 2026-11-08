//! Accepting witnesses checked by an independent finite closed-walk oracle.
#![allow(clippy::expect_used, reason = "test assertions")]

use std::collections::{BTreeSet, VecDeque};

use crate::Graph;
use crate::search::{Acceptance, Lasso, find_lasso};

// This reference explores (vertex, accumulated visit mask) directly. It
// uses no SCCs, component acceptance test or covering-walk heuristic.
fn reference(edges: &[Vec<usize>], roots: &[usize], labels: &Acceptance) -> bool {
    let mut reachable = BTreeSet::new();
    let mut todo: VecDeque<_> = roots.iter().copied().collect();
    while let Some(v) = todo.pop_front() {
        if reachable.insert(v) {
            todo.extend(edges[v].iter().copied());
        }
    }
    let state_bits = |v| {
        labels
            .states
            .iter()
            .enumerate()
            .fold(0_u32, |bits, (i, row)| {
                bits | if row[v] { 1 << i } else { 0 }
            })
    };
    let edge_bits = |v: usize, e: usize| {
        labels
            .actions
            .iter()
            .enumerate()
            .fold(0_u32, |bits, (i, rows)| {
                bits | if rows[v][e] {
                    1 << (labels.states.len() + i)
                } else {
                    0
                }
            })
    };
    let required = (1 << (labels.states.len() + labels.actions.len())) - 1;
    for &entry in &reachable {
        let mut seen = BTreeSet::new();
        let mut queue = VecDeque::from([(entry, state_bits(entry))]);
        while let Some((v, mask)) = queue.pop_front() {
            if !seen.insert((v, mask)) {
                continue;
            }
            for (e, &to) in edges[v].iter().enumerate() {
                if !labels.recurrent[v][e] {
                    continue;
                }
                let mask = mask | edge_bits(v, e) | state_bits(to);
                if to == entry && mask == required {
                    return true;
                }
                queue.push_back((to, mask));
            }
        }
    }
    false
}

fn check_witness(edges: &[Vec<usize>], roots: &[usize], labels: &Acceptance, lasso: &Lasso) {
    assert!(roots.contains(&lasso.root));
    let mut here = lasso.root;
    for step in &lasso.prefix {
        assert_eq!(step.from, here);
        assert_eq!(edges[step.from][step.edge], step.to);
        here = step.to;
    }
    assert_eq!(here, lasso.entry);
    assert_ne!(lasso.cycle, Vec::new());
    let mut vertices = BTreeSet::from([here]);
    for step in &lasso.cycle {
        assert_eq!(step.from, here);
        assert_eq!(edges[step.from][step.edge], step.to);
        assert!(labels.recurrent[step.from][step.edge]);
        vertices.insert(step.to);
        here = step.to;
    }
    assert_eq!(here, lasso.entry);
    for row in &labels.states {
        assert!(vertices.iter().any(|v| row[*v]));
    }
    for rows in &labels.actions {
        assert!(lasso.cycle.iter().any(|e| rows[e.from][e.edge]));
    }
    // Prefix length independently checked by ordinary BFS, not by the
    // canonical frontier rule used by the implementation.
    let mut distance = vec![None; edges.len()];
    let mut queue = VecDeque::new();
    for &v in roots {
        distance[v] = Some(0);
        queue.push_back(v);
    }
    while let Some(v) = queue.pop_front() {
        for &to in &edges[v] {
            if distance[to].is_none() {
                distance[to] = distance[v].map(|d| d + 1);
                queue.push_back(to);
            }
        }
    }
    assert_eq!(distance[lasso.entry], Some(lasso.prefix.len()));
}

#[test]
fn all_two_vertex_edge_and_visit_labels_match_closed_walk_search() {
    // Three choices per possible edge: absent, prefix-only, recurrent.
    // Two independent state conditions and one edge condition.
    for encoding in 0..81_u32 {
        let mut edges = vec![vec![]; 2];
        let mut recurrent = vec![vec![]; 2];
        let mut code = encoding;
        for v in 0..2 {
            for to in 0..2 {
                let digit = code % 3;
                code /= 3;
                if digit != 0 {
                    edges[v].push(to);
                    recurrent[v].push(digit == 2);
                }
            }
        }
        for state_mask in 0..16_u32 {
            let states: Vec<Vec<bool>> = (0..2)
                .map(|i| {
                    (0..2)
                        .map(|v| state_mask & (1 << (i * 2 + v)) != 0)
                        .collect()
                })
                .collect();
            for edge_mask in 0..16_u32 {
                let actions = vec![
                    edges
                        .iter()
                        .enumerate()
                        .map(|(v, row)| {
                            row.iter()
                                .map(|to| edge_mask & (1 << (v * 2 + to)) != 0)
                                .collect()
                        })
                        .collect(),
                ];
                let labels = Acceptance {
                    recurrent: recurrent.clone(),
                    states: states.clone(),
                    actions,
                };
                let graph = Graph::new(edges.clone(), vec![0]).expect("graph");
                let result = find_lasso(&graph, &labels).expect("search");
                assert_eq!(
                    result.is_some(),
                    reference(&edges, &[0], &labels),
                    "graph {encoding}, states {state_mask}, actions {edge_mask}"
                );
                if let Some(lasso) = result {
                    check_witness(&edges, &[0], &labels, &lasso);
                }
            }
        }
    }
}

#[test]
fn finite_prefix_is_unrestricted_and_unreachable_cycles_are_excluded() {
    let edges = vec![vec![1], vec![2], vec![1], vec![3]];
    let graph = Graph::new(edges.clone(), vec![0]).expect("graph");
    let labels = Acceptance {
        recurrent: vec![vec![false], vec![true], vec![true], vec![true]],
        states: vec![
            vec![false, true, false, true],
            vec![false, false, true, false],
        ],
        actions: vec![],
    };
    let lasso = find_lasso(&graph, &labels).expect("search").expect("lasso");
    assert_eq!(
        (lasso.entry, lasso.prefix.len(), lasso.cycle.len()),
        (1, 1, 2)
    );
    check_witness(&edges, &[0], &labels, &lasso);
    let labels = Acceptance {
        states: vec![vec![false, false, false, true]],
        ..labels
    };
    assert_eq!(find_lasso(&graph, &labels).expect("search"), None);
}

#[test]
fn parallel_edges_keep_their_action_labels_and_positions() {
    let edges = vec![vec![0, 0, 0]];
    let graph = Graph::new(edges.clone(), vec![0]).expect("graph");
    let labels = Acceptance {
        recurrent: vec![vec![false, true, true]],
        states: vec![vec![true]],
        actions: vec![
            vec![vec![true, true, false]],
            vec![vec![false, false, true]],
        ],
    };
    let lasso = find_lasso(&graph, &labels).expect("search").expect("lasso");
    assert_eq!(
        lasso.cycle.iter().map(|s| s.edge).collect::<Vec<_>>(),
        [1, 2]
    );
    check_witness(&edges, &[0], &labels, &lasso);
}

#[test]
fn shortest_prefix_ties_use_vertex_ids_and_sorted_frontiers() {
    let graph = Graph::new(vec![vec![2, 1], vec![3], vec![3], vec![3]], vec![0]).expect("graph");
    let labels = Acceptance {
        recurrent: vec![vec![false; 2], vec![false], vec![false], vec![true]],
        states: vec![],
        actions: vec![],
    };
    let lasso = find_lasso(&graph, &labels).expect("search").expect("lasso");
    assert_eq!(
        lasso.prefix.iter().map(|s| s.to).collect::<Vec<_>>(),
        [1, 3]
    );
    let roots = Graph::new(vec![vec![0], vec![1]], vec![1, 0, 1]).expect("roots");
    let labels = Acceptance {
        recurrent: vec![vec![true]; 2],
        states: vec![],
        actions: vec![],
    };
    assert_eq!(
        find_lasso(&roots, &labels)
            .expect("search")
            .expect("lasso")
            .entry,
        0
    );
}

#[test]
fn empty_conditions_still_require_a_nonempty_cycle() {
    let graph = Graph::new(vec![vec![1], vec![0]], vec![0]).expect("graph");
    let labels = Acceptance {
        recurrent: vec![vec![true]; 2],
        states: vec![],
        actions: vec![],
    };
    assert_eq!(
        find_lasso(&graph, &labels)
            .expect("search")
            .expect("lasso")
            .cycle
            .len(),
        2
    );
    let empty = Graph::new(vec![], vec![]).expect("empty");
    let labels = Acceptance {
        recurrent: vec![],
        states: vec![],
        actions: vec![],
    };
    assert_eq!(find_lasso(&empty, &labels).expect("search"), None);
}

#[test]
fn malformed_label_tables_never_become_no_violation() {
    let graph = Graph::new(vec![vec![0]], vec![0]).expect("graph");
    for labels in [
        Acceptance {
            recurrent: vec![],
            states: vec![],
            actions: vec![],
        },
        Acceptance {
            recurrent: vec![vec![true]],
            states: vec![vec![]],
            actions: vec![],
        },
        Acceptance {
            recurrent: vec![vec![true]],
            states: vec![],
            actions: vec![vec![vec![]]],
        },
    ] {
        assert!(find_lasso(&graph, &labels).is_err());
    }
}

#[test]
fn components_compete_by_prefix_distance_then_entry_id() {
    let edges = vec![vec![3, 1], vec![2], vec![1], vec![3], vec![4]];
    let graph = Graph::new(edges.clone(), vec![0]).expect("graph");
    let mut labels = Acceptance {
        recurrent: vec![
            vec![false; 2],
            vec![true],
            vec![true],
            vec![true],
            vec![true],
        ],
        states: vec![vec![false, true, false, true, true]],
        actions: vec![],
    };
    let lasso = find_lasso(&graph, &labels).expect("search").expect("lasso");
    assert_eq!(lasso.entry, 1); // Same prefix length as 3; lower vertex id.
    check_witness(&edges, &[0], &labels, &lasso);
    labels.states.push(vec![false, false, false, true, true]);
    let lasso = find_lasso(&graph, &labels).expect("search").expect("lasso");
    assert_eq!(lasso.entry, 3); // Conditions cannot be split between SCCs.
    assert_eq!(lasso.cycle.len(), 1);
    check_witness(&edges, &[0], &labels, &lasso);
}

#[test]
fn documented_prefix_then_two_cycle_witness() {
    let graph = Graph::new(vec![vec![1], vec![2], vec![1]], vec![0]).expect("graph");
    let labels = Acceptance {
        recurrent: vec![vec![false], vec![true], vec![true]],
        states: vec![vec![false, true, false], vec![false, false, true]],
        actions: vec![],
    };
    let lasso = find_lasso(&graph, &labels)
        .expect("search")
        .expect("accepting loop");
    assert_eq!((lasso.root, lasso.entry), (0, 1));
    assert_eq!(lasso.prefix.len(), 1);
    assert_eq!(lasso.cycle.iter().map(|e| e.to).collect::<Vec<_>>(), [2, 1]);
}
