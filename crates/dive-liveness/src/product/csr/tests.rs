//! CSR storage controls; no model/evaluator or large allocation is needed.
#![allow(clippy::expect_used, reason = "finite row assertions")]

use super::*;

#[test]
fn canonical_rows_keep_duplicates_original_positions_and_empty_rows() {
    let mut flat = FlatRows::default();
    let mut owned = OwnedRows::default();
    for _ in 0..4 {
        flat.add_vertex().expect("vertex");
        owned.add_vertex().expect("vertex");
    }
    for (v, row) in [vec![(2, 3), (2, 7)], vec![], vec![(0, 1)], vec![]]
        .iter()
        .enumerate()
    {
        for &(target, original) in row {
            flat.push_edge(v, target, original).expect("edge");
            owned.push_edge(v, target, original).expect("edge");
        }
        flat.end_row(v).expect("row");
        owned.end_row(v).expect("row");
    }
    // Discovery order differs from canonical (behavior, tableau) pair order.
    let order = [2, 0, 3, 1];
    let renumber = [1, 3, 0, 2];
    let flat = flat.finish(&order, &renumber, vec![0, 1]).expect("CSR");
    let owned = owned.finish(&order, &renumber, vec![0, 1]).expect("owned");
    assert_eq!(flat.owned_graph(), owned.graph);
    assert_eq!(flat.owned_original_edges(), owned.original_edges);
    assert_eq!(canonical_offsets(&flat), [0, 1, 3, 3, 3]);
    assert_eq!(canonical_targets(&flat), [1, 0, 0]);
    assert_eq!(canonical_originals(&flat), [1, 3, 7]);
    assert_eq!(flat.ranges, [2..3, 0..2, 3..3, 2..2]);
    assert_eq!(flat.targets, [0, 0, 1]);
    assert_eq!(flat.original_edges, [3, 7, 1]);
}

#[test]
fn empty_discovery_and_root_only_zero_edge_rows_are_retained() {
    let empty = FlatRows::default().finish(&[], &[], vec![]).expect("empty");
    assert_eq!(canonical_offsets(&empty), [0]);
    assert_eq!(empty.ranges, [] as [std::ops::Range<usize>; 0]);
    assert_eq!(empty.vertex_count(), 0);
    let mut rows = FlatRows::default();
    for _ in 0..65 {
        rows.add_vertex().expect("root");
    }
    for v in 0..65 {
        rows.end_row(v).expect("empty row");
    }
    let order: Vec<_> = (0..65).collect();
    let rows = rows.finish(&order, &order, order.clone()).expect("roots");
    assert_eq!(canonical_offsets(&rows), vec![0; 66]);
    assert_eq!(rows.ranges, vec![0..0; 65]);
    assert_eq!(rows.roots(), order);
    assert_eq!(rows.vertex_count(), 65);
}

#[test]
fn malformed_offsets_lengths_targets_and_roots_fail_closed() {
    for (offsets, targets, originals, roots) in [
        (vec![], vec![], vec![], vec![]),
        (vec![1], vec![], vec![], vec![]),
        (vec![0, 1, 0], vec![], vec![], vec![]),
        (vec![0, 2], vec![0], vec![0], vec![]),
        (vec![0, 1], vec![0], vec![], vec![]),
        (vec![0, 1], vec![1], vec![0], vec![]),
        (vec![0, 0], vec![], vec![], vec![1]),
    ] {
        assert!(matches!(
            CsrGraph::checked(offsets, targets, originals, roots),
            Err(ProductError::Shape(_))
        ));
    }
}

#[test]
fn incomplete_and_out_of_order_expansion_is_never_a_graph() {
    let mut rows = FlatRows::default();
    rows.add_vertex().expect("vertex");
    assert!(matches!(rows.end_row(1), Err(ProductError::Shape(_))));
    assert!(matches!(
        rows.push_edge(1, 0, 0),
        Err(ProductError::Shape(_))
    ));
    assert!(matches!(
        rows.push_edge(0, 1, 0),
        Err(ProductError::Shape(_))
    ));
    assert!(matches!(
        rows.finish(&[0], &[0], vec![0]),
        Err(ProductError::Shape(_))
    ));
}

#[test]
fn byte_overflow_and_impossible_reservations_fail_before_writes() {
    assert!(matches!(
        reserve::<usize>(usize::MAX),
        Err(ProductError::Limit("product adjacency bytes"))
    ));
    assert!(matches!(
        reserve::<u8>(usize::MAX),
        Err(ProductError::Limit("product adjacency allocation"))
    ));
    let mut rows = Vec::<usize>::new();
    assert_eq!(
        grow(&mut rows, usize::MAX),
        Err(ProductError::Limit("product adjacency bytes"))
    );
    assert_eq!(rows, [] as [usize; 0]);
    assert_eq!(rows.capacity(), 0);
}

#[test]
fn finalization_budget_trip_is_control_flow_not_empty_language() {
    let mut rows = FlatRows::default();
    rows.add_vertex().expect("vertex");
    rows.push_edge(0, 0, 0).expect("edge");
    rows.end_row(0).expect("row");
    let watchdog = crate::budget::expired_for_test();
    let _scope = crate::budget::enter(watchdog.budget());
    assert!(matches!(
        rows.finish(&[0], &[0], vec![0]),
        Err(ProductError::Limit(crate::budget::EXHAUSTED))
    ));
}

fn canonical_offsets(graph: &impl GraphAccess) -> Vec<usize> {
    let mut offsets = vec![0];
    let mut end = 0;
    for v in 0..graph.vertex_count() {
        end += graph.successors(v).len();
        offsets.push(end);
    }
    offsets
}

fn canonical_targets(graph: &impl GraphAccess) -> Vec<usize> {
    (0..graph.vertex_count())
        .flat_map(|v| graph.successors(v).iter().copied())
        .collect()
}

fn canonical_originals(graph: &impl ProductRows) -> Vec<usize> {
    (0..graph.vertex_count())
        .flat_map(|v| graph.original_edges(v).iter().copied())
        .collect()
}

#[test]
fn malformed_canonical_permutations_and_inverses_fail_closed() {
    for (order, inverse) in [
        (vec![0], vec![0, 1]),
        (vec![0, 1], vec![0]),
        (vec![0, 0], vec![0, 1]),
        (vec![0, 2], vec![0, 1]),
        (vec![1, 0], vec![0, 1]),
        (vec![0, 1], vec![0, 2]),
    ] {
        assert!(matches!(
            CsrGraph::from_discovery(
                vec![0, 1, 2],
                vec![0, 1],
                vec![7, 9],
                vec![0],
                &order,
                &inverse
            ),
            Err(ProductError::Shape(_))
        ));
    }
    let empty_rows =
        CsrGraph::from_discovery(vec![0, 0, 0], vec![], vec![], vec![0], &[1, 0], &[1, 0])
            .expect("valid empty row permutation");
    assert_eq!(empty_rows.ranges, [0..0, 0..0]);
}

#[test]
fn nonempty_discovery_payloads_move_without_reallocation() {
    let mut rows = FlatRows::default();
    for _ in 0..4 {
        rows.add_vertex().expect("vertex");
    }
    for (v, row) in [vec![(2, 3), (2, 7)], vec![], vec![(0, 1)], vec![]]
        .iter()
        .enumerate()
    {
        for &(target, original) in row {
            rows.push_edge(v, target, original).expect("edge");
        }
        rows.end_row(v).expect("row");
    }
    assert_eq!(rows.targets.len(), 3);
    assert_eq!(rows.original_edges.len(), 3);
    let target_pointer = rows.targets.as_ptr();
    let original_pointer = rows.original_edges.as_ptr();
    let target_capacity = rows.targets.capacity();
    let original_capacity = rows.original_edges.capacity();
    let graph = rows
        .finish(&[2, 0, 3, 1], &[1, 3, 0, 2], vec![0])
        .expect("moved buffers");
    assert_eq!(graph.targets.as_ptr(), target_pointer);
    assert_eq!(graph.original_edges.as_ptr(), original_pointer);
    assert_eq!(graph.targets.capacity(), target_capacity);
    assert_eq!(graph.original_edges.capacity(), original_capacity);
    assert_eq!(canonical_targets(&graph), [1, 0, 0]);
    assert_eq!(canonical_originals(&graph), [1, 3, 7]);
}
