//! Partial SCC input/write authority and independent accepting-cycle controls.
#![allow(
    clippy::expect_used,
    clippy::panic,
    reason = "adversarial test assertions"
)]
use dive_liveness::{Graph, product::Limits, search::Acceptance, tlc_scc, tlc_witness};
fn labels(edges: &[Vec<usize>]) -> Acceptance {
    Acceptance {
        recurrent: edges.iter().map(|row| vec![true; row.len()]).collect(),
        states: vec![],
        actions: vec![],
    }
}
fn independently_accepts(edges: &[Vec<usize>], roots: &[usize], a: &Acceptance) -> bool {
    let n = edges.len();
    let closure = |restricted: bool| {
        let mut reach = vec![vec![false; n]; n];
        for i in 0..n {
            reach[i][i] = true;
            for (e, &target) in edges[i].iter().enumerate() {
                reach[i][target] |= !restricted || a.recurrent[i][e];
            }
        }
        for k in 0..n {
            for i in 0..n {
                for j in 0..n {
                    reach[i][j] |= reach[i][k] && reach[k][j];
                }
            }
        }
        reach
    };
    let full = closure(false);
    let restricted = closure(true);
    for anchor in 0..n {
        if !roots.iter().any(|r| full[*r][anchor]) {
            continue;
        }
        let members: Vec<_> = (0..n)
            .filter(|i| restricted[anchor][*i] && restricted[*i][anchor])
            .collect();
        let inside = |id| members.contains(&id);
        let cyclic = members.len() > 1
            || edges[anchor]
                .iter()
                .enumerate()
                .any(|(e, t)| *t == anchor && a.recurrent[anchor][e]);
        if !cyclic {
            continue;
        }
        let states = a.states.iter().all(|row| members.iter().any(|id| row[*id]));
        let actions = a.actions.iter().all(|rows| {
            members.iter().any(|id| {
                edges[*id]
                    .iter()
                    .enumerate()
                    .any(|(e, target)| inside(*target) && a.recurrent[*id][e] && rows[*id][e])
            })
        });
        if states && actions {
            return true;
        }
    }
    false
}

#[test]
fn unfinished_roots_and_targets_do_not_become_empty_completed_components() {
    let edges = vec![vec![1, 2], vec![1], vec![2]];
    let graph = Graph::new(edges.clone(), vec![0, 0]).expect("graph");
    let mut a = labels(&edges);
    let empty_graph =
        Graph::new(vec![vec![], vec![], vec![]], vec![0, 0]).expect("unwritten graph");
    let empty_labels = labels(&[vec![], vec![], vec![]]);
    let empty = tlc_scc::check_partial(&empty_graph, &empty_labels, &[false; 3], Limits::default())
        .expect("unwritten roots");
    assert_eq!(empty.traversal().discovery, Vec::<usize>::new());
    assert_eq!(
        empty.traversal().components,
        Vec::<tlc_scc::Component>::new()
    );
    assert_eq!((empty.written(), empty.registered()), (0, 3));
    assert!(!empty.model_proof());
    let first_graph =
        Graph::new(vec![vec![1, 2], vec![], vec![]], vec![0, 0]).expect("partial storage");
    let first_labels = labels(&[vec![1, 2], vec![], vec![]]);
    let first = tlc_scc::check_partial(
        &first_graph,
        &first_labels,
        &[true, false, false],
        Limits::default(),
    )
    .expect("undone children");
    assert_eq!(first.traversal().discovery, [0]);
    assert_eq!(first.traversal().accepting, None);
    a.recurrent[0][1] = false;
    a.states = vec![vec![false, false, true]];
    let grown = tlc_scc::check_partial(&graph, &a, &[true, true, true], Limits::default())
        .expect("grown graph");
    assert_eq!(grown.traversal().discovery, [0, 1, 2]);
    assert_eq!(
        grown
            .traversal()
            .components
            .last()
            .expect("component")
            .anchor,
        2
    );
    assert_eq!(grown.traversal().accepting, Some(2));
    assert!(!grown.model_proof());
    let original = tlc_scc::check(&graph, &a, Limits::default()).expect("same complete input");
    assert_eq!(grown.traversal().discovery, original.discovery);
    assert_eq!(grown.traversal().components, original.components);
    assert_eq!(grown.traversal().accepting, original.accepting);
    // Partial mode additionally reserves and inspects every write-mask cell;
    // that work must remain charged rather than hidden to match final mode.
    assert_eq!(
        grown.traversal().inspections,
        original.inspections + 3 * edges.len()
    );
    assert_eq!(
        grown,
        tlc_scc::check_partial(&graph, &a, &[true; 3], Limits::default())
            .expect("restored fresh links")
    );
}
#[test]
fn partial_dimensions_and_limits_refuse_instead_of_claiming_holds() {
    let graph = Graph::new(vec![vec![0]], vec![0]).expect("graph");
    let a = labels(&[vec![0]]);
    assert!(matches!(
        tlc_scc::check_partial(&graph, &a, &[], Limits::default()),
        Err(tlc_scc::Error::Input(_))
    ));
    for steps in 0..8 {
        assert!(matches!(
            tlc_scc::check_partial(
                &graph,
                &a,
                &[false],
                Limits {
                    steps,
                    ..Limits::default()
                }
            ),
            Err(tlc_scc::Error::Limit(_))
        ));
    }
    let bad = Acceptance {
        states: vec![vec![]],
        ..a.clone()
    };
    assert!(matches!(
        tlc_scc::check_partial(&graph, &bad, &[false], Limits::default()),
        Err(tlc_scc::Error::Input(_))
    ));
    let ids = [tlc_witness::Identity {
        fingerprint: 0,
        tableau: -1,
    }];
    let empty_graph = Graph::new(vec![vec![]], vec![0]).expect("empty storage");
    let empty_labels = labels(&[vec![]]);
    let selected = tlc_witness::select_partial(
        &empty_graph,
        &empty_labels,
        &ids,
        &[false],
        Limits::default(),
    )
    .expect("partial no candidate");
    assert!(selected.candidate().is_none());
    assert!(!selected.model_proof());
    assert!(selected.inspections() > selected.traversal().inspections);
    assert!(tlc_witness::select_partial(&graph, &a, &[], &[false], Limits::default()).is_err());
}
#[test]
#[allow(
    clippy::too_many_lines,
    reason = "keep the complete independent graph/write-mask census and paired refusal controls together"
)]
fn all_three_node_write_masks_match_independent_floyd_acceptance() {
    let mut total = 0;
    for mask in 0u32..512 {
        let edges: Vec<Vec<usize>> = (0..3)
            .map(|i| (0..3).filter(|j| mask & (1 << (i * 3 + j)) != 0).collect())
            .collect();
        for sample in 0..8 {
            let mut a = labels(&edges);
            for (i, row) in edges.iter().enumerate() {
                for (e, &target) in row.iter().enumerate() {
                    a.recurrent[i][e] = (i + target + sample) % 3 != 0;
                }
            }
            a.states = vec![(0..3).map(|i| (i + sample) % 2 == 0).collect()];
            a.actions = vec![
                edges
                    .iter()
                    .enumerate()
                    .map(|(i, row)| row.iter().map(|t| (i + t + sample) % 2 == 1).collect())
                    .collect(),
            ];
            let roots = if sample % 2 == 0 {
                vec![0, 1, 0]
            } else {
                vec![2]
            };
            let graph = Graph::new(edges.clone(), roots.clone()).expect("graph");
            for bits in 0..8 {
                let written: Vec<bool> = (0..3).map(|i| bits & (1 << i) != 0).collect();
                let effective: Vec<Vec<usize>> = edges
                    .iter()
                    .enumerate()
                    .map(|(i, row)| {
                        row.iter()
                            .filter(|t| written[i] && written[**t])
                            .copied()
                            .collect()
                    })
                    .collect();
                let effective_roots: Vec<_> =
                    roots.iter().filter(|r| written[**r]).copied().collect();
                let mut independent = labels(&effective);
                independent.states = a.states.clone();
                independent.actions = vec![
                    effective
                        .iter()
                        .enumerate()
                        .map(|(i, row)| row.iter().map(|t| (i + t + sample) % 2 == 1).collect())
                        .collect(),
                ];
                for (i, row) in effective.iter().enumerate() {
                    for (e, &target) in row.iter().enumerate() {
                        independent.recurrent[i][e] = (i + target + sample) % 3 != 0;
                    }
                }
                let stored: Vec<Vec<usize>> = edges
                    .iter()
                    .enumerate()
                    .map(|(i, row)| if written[i] { row.clone() } else { vec![] })
                    .collect();
                let stored_graph =
                    Graph::new(stored.clone(), roots.clone()).expect("stored partial graph");
                let mut stored_labels = labels(&stored);
                stored_labels.states = a.states.clone();
                stored_labels.actions = vec![
                    stored
                        .iter()
                        .enumerate()
                        .map(|(i, row)| row.iter().map(|t| (i + t + sample) % 2 == 1).collect())
                        .collect(),
                ];
                for (i, row) in stored.iter().enumerate() {
                    for (e, &target) in row.iter().enumerate() {
                        stored_labels.recurrent[i][e] = (i + target + sample) % 3 != 0;
                    }
                }
                if edges
                    .iter()
                    .enumerate()
                    .any(|(i, row)| !written[i] && !row.is_empty())
                {
                    assert!(matches!(
                        tlc_scc::check_partial(&graph, &a, &written, Limits::default()),
                        Err(tlc_scc::Error::Input(_))
                    ));
                }
                let run = tlc_scc::check_partial(
                    &stored_graph,
                    &stored_labels,
                    &written,
                    Limits::default(),
                )
                .expect("partial SCC");
                assert_eq!(
                    run.traversal().accepting.is_some(),
                    independently_accepts(&effective, &effective_roots, &independent),
                    "graph={mask} sample={sample} written={bits}"
                );
                assert!(run.traversal().discovery.iter().all(|i| written[*i]));
                assert!(!run.model_proof());
                let ids: Vec<_> = (0u64..3)
                    .map(|fingerprint| tlc_witness::Identity {
                        fingerprint,
                        tableau: -1,
                    })
                    .collect();
                let selected = tlc_witness::select_partial(
                    &stored_graph,
                    &stored_labels,
                    &ids,
                    &written,
                    Limits::default(),
                )
                .expect("partial selection");
                assert_eq!(selected.traversal(), run.traversal());
                if let Some(c) = selected.candidate() {
                    assert!(written[c.anchor]);
                    assert!(c.cycle_stack.iter().all(|i| written[*i]));
                    assert!(written[c.postfix_end]);
                }
                total += 1;
            }
        }
    }
    assert_eq!(total, 32768);
}

#[test]
fn supplied_unwritten_adjacency_cannot_shortcut_a_real_written_prefix() {
    let invented = vec![vec![1, 3], vec![2], vec![2], vec![4], vec![2]];
    let graph = Graph::new(invented.clone(), vec![0]).expect("graph");
    let a = labels(&invented);
    let written = [true, false, true, true, true];
    let ids: Vec<_> = (0u64..5)
        .map(|fingerprint| tlc_witness::Identity {
            fingerprint,
            tableau: -1,
        })
        .collect();
    assert!(matches!(
        tlc_witness::select_partial(&graph, &a, &ids, &written, Limits::default()),
        Err(tlc_scc::Error::Input(_))
    ));
    let stored = vec![vec![1, 3], vec![], vec![2], vec![4], vec![2]];
    let graph = Graph::new(stored.clone(), vec![0]).expect("stored graph");
    let selected =
        tlc_witness::select_partial(&graph, &labels(&stored), &ids, &written, Limits::default())
            .expect("actual written path");
    assert_eq!(
        selected
            .candidate()
            .expect("accepting cycle")
            .prefix_reverse,
        [2, 4, 3, 0]
    );
    assert!(!selected.model_proof());
}
