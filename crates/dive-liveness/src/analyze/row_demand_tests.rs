//! Cache completeness and physical prefix maps, without state-equality lookup.
#![allow(clippy::expect_used, reason = "explicit graph/cache controls")]

use super::*;
use std::sync::Arc;

#[test]
fn incomplete_and_duplicate_rows_never_supply_default_false_truth() {
    let mut cache = Cache {
        active: true,
        rows: Vec::new(),
        offsets: vec![0, 1],
        width: 2,
    };
    assert!(cache.insert(0, &[Some(true), None]).is_err());
    assert!(!cache.complete(0));
    assert!(cache.read(0, 0, 0).is_err());
    cache
        .insert(0, &[Some(false), Some(true)])
        .expect("complete row");
    assert!(!cache.read(0, 0, 0).expect("actual false"));
    assert!(cache.read(1, 0, 0).expect("actual true"));
    assert!(
        cache.read(0, 1, 0).is_err(),
        "cannot read another property's slot"
    );
    assert!(cache.insert(0, &[Some(true), Some(false)]).is_err());
    assert!(!cache.read(0, 0, 0).expect("original row remains"));
}

#[test]
fn frontier_projection_preserves_duplicate_edge_occurrences_and_original_ids() {
    let state = || Arc::from([Some(dive_value::Value::Int(0))]);
    let graph = capture::BehaviorGraph {
        // Equal values deliberately do not identify original vertices.
        states: vec![state(), state(), state()],
        roots: vec![0],
        edges: vec![
            vec![
                capture::Edge {
                    to: 1,
                    ordinal: Some(0),
                    label: Some("frontier".into()),
                },
                capture::Edge {
                    to: 2,
                    ordinal: Some(1),
                    label: Some("first".into()),
                },
                capture::Edge {
                    to: 2,
                    ordinal: Some(2),
                    label: Some("second".into()),
                },
                capture::Edge {
                    to: 0,
                    ordinal: None,
                    label: None,
                },
            ],
            Vec::new(),
            vec![capture::Edge {
                to: 2,
                ordinal: None,
                label: None,
            }],
        ],
        concrete: vec![Vec::new(), Vec::new(), Vec::new()],
    };
    let cache = Cache {
        active: true,
        rows: vec![Some(vec![1]), None, Some(vec![0])],
        offsets: vec![0],
        width: 1,
    };
    let projection = Projection::new(&graph, &cache).expect("expanded projection");
    assert_eq!(projection.sources, vec![0, 2]);
    assert_eq!(projection.edges, vec![vec![1, 2, 3], vec![0]]);
    assert_eq!(
        projection.graph,
        crate::Graph::new(vec![vec![1, 1, 0], vec![1]], vec![0]).expect("expected topology")
    );
    assert!(cache.read(0, 0, projection.sources[0]).expect("source0"));
    assert!(!cache.read(0, 0, projection.sources[1]).expect("source2"));
    for (source, positions) in projection.edges.iter().enumerate() {
        for (edge, original) in positions.iter().enumerate() {
            assert_eq!(
                projection.graph.edges[source][edge],
                projection
                    .sources
                    .iter()
                    .position(|&id| id == graph.edges[projection.sources[source]][*original].to)
                    .expect("original destination")
            );
        }
    }
}

#[test]
fn disabling_discards_all_rows_and_cannot_supply_projection_or_truth() {
    let mut cache = Cache {
        active: true,
        rows: Vec::new(),
        offsets: vec![0],
        width: 1,
    };
    cache.insert(0, &[Some(true)]).expect("first actual row");
    cache.insert(1, &[Some(false)]).expect("second actual row");
    assert!(cache.complete(0) && cache.complete(1));
    cache.disable();
    assert!(!cache.active());
    assert!(cache.rows.is_empty());
    assert!(!cache.complete(0) && !cache.complete(1));
    assert!(cache.read(0, 0, 0).is_err());
    assert!(cache.read(0, 0, 1).is_err());
    assert!(cache.insert(2, &[Some(true)]).is_err());
    let graph = capture::BehaviorGraph {
        states: vec![Arc::from([Some(dive_value::Value::Int(0))])],
        roots: vec![0],
        edges: vec![Vec::new()],
        concrete: vec![Vec::new()],
    };
    assert!(Projection::new(&graph, &cache).is_err());
    cache.disable();
    assert!(!cache.active(), "disable is monotone");
}
