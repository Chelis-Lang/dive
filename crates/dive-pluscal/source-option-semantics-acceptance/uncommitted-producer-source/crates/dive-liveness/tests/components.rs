//! SCCs checked against mutual reachability, independently of Tarjan traversal.
#![allow(clippy::expect_used, reason = "test assertions")]

use dive_liveness::{Graph, GraphError};

#[test]
fn all_four_vertex_graphs_match_mutual_reachability() {
    let n = 4;
    for bits in 0..(1_u32 << (n * n)) {
        let edges: Vec<Vec<usize>> = (0..n)
            .map(|i| (0..n).filter(|j| bits & (1 << (i * n + j)) != 0).collect())
            .collect();
        let mut reach = vec![vec![false; n]; n];
        for i in 0..n {
            reach[i][i] = true;
            for &j in &edges[i] {
                reach[i][j] = true;
            }
        }
        for k in 0..n {
            for i in 0..n {
                for j in 0..n {
                    reach[i][j] |= reach[i][k] && reach[k][j];
                }
            }
        }
        for root in 0..n {
            let mut used = vec![false; n];
            let mut expected = Vec::new();
            for i in 0..n {
                if used[i] || !reach[root][i] {
                    continue;
                }
                let members: Vec<usize> = (0..n).filter(|j| reach[i][*j] && reach[*j][i]).collect();
                for &j in &members {
                    used[j] = true;
                }
                expected.push(members);
            }
            assert_eq!(
                Graph::new(edges.clone(), vec![root])
                    .expect("valid")
                    .components(),
                expected,
                "bits {bits}, root {root}"
            );
        }
    }
}

#[test]
fn only_reachable_cycles_are_candidates_and_order_is_stable() {
    let edges = vec![vec![2, 1, 1], vec![2], vec![1], vec![3]];
    let graph = Graph::new(edges.clone(), vec![0, 0]).expect("graph");
    assert_eq!(graph.components(), [vec![0], vec![1, 2]]);
    assert_eq!(graph.cyclic_components(), [vec![1, 2]]);
    let reversed: Vec<Vec<usize>> = edges
        .into_iter()
        .map(|mut e| {
            e.reverse();
            e
        })
        .collect();
    assert_eq!(
        Graph::new(reversed, vec![0]).expect("graph").components(),
        graph.components()
    );
    assert_eq!(
        Graph::new(vec![vec![0]], vec![0])
            .expect("self-loop")
            .cyclic_components(),
        [vec![0]]
    );
    assert_eq!(
        Graph::new(vec![], vec![]).expect("empty").components(),
        Vec::<Vec<usize>>::new()
    );
}

#[test]
fn malformed_references_fail_before_search() {
    assert_eq!(
        Graph::new(vec![vec![1]], vec![0]),
        Err(GraphError {
            vertex: 1,
            source: Some(0)
        })
    );
    assert_eq!(
        Graph::new(vec![vec![]], vec![1]),
        Err(GraphError {
            vertex: 1,
            source: None
        })
    );
}

#[test]
fn deep_cycle_uses_an_explicit_stack() {
    let n = 50_000;
    let edges = (0..n).map(|i| vec![(i + 1) % n]).collect();
    let graph = Graph::new(edges, vec![0]).expect("deep cycle");
    assert_eq!(graph.components(), [(0..n).collect::<Vec<_>>()]);
}

#[test]
fn eventual_cycle_constraints_do_not_constrain_the_prefix() {
    // LvEAPrefix: TLC finds the stuttering cycle at x = 1 even though x > 0
    // is false in its initial state. The component at 3 is unreachable.
    let edges = vec![vec![0, 1], vec![1, 2], vec![2, 1], vec![3]];
    let graph = Graph::new(edges.clone(), vec![0]).expect("graph");
    let recurrent = graph.filter_for_cycles(|from, _| from > 0);
    assert_eq!(recurrent.cyclic_components(), [vec![1, 2]]);
    let mut wrongly_pruned = edges;
    wrongly_pruned[0].clear();
    assert_eq!(
        Graph::new(wrongly_pruned, vec![0])
            .expect("graph")
            .cyclic_components(),
        Vec::<Vec<usize>>::new()
    );
}
