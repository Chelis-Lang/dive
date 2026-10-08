//! dive's seeding (D-002, D-003): a run replays from its seed at any worker count.
//!
//! A random state graph (as `RndBfs`: `x' = (3x + d * i) % 61` with `d` drawn per
//! state) is explored level by level, with the frontier split over 1 to 8 threads and
//! each thread taking its states in a scrambled order. The draws, and so the graph,
//! must be identical every time, and equal to a sequential run.

#![allow(
    clippy::unwrap_used,
    clippy::many_single_char_names,
    reason = "test code; short names mirror the TLA+ specs"
)]

use std::collections::BTreeSet;

use dive_stdlib::random::{RandomMode, RandomSource, Streams, random_element};
use dive_value::fp64::{Fingerprinter, TlcFingerprint};
use dive_value::{Ctx, Int, Value};

fn successors(streams: Streams, x: i64) -> Vec<i64> {
    let ctx = Ctx::dive();
    let fp = TlcFingerprint::new(0)
        .unwrap()
        .fingerprint_state(&[Value::from(x)], ctx)
        .unwrap();
    let mut s = streams.state(fp);
    let digits = Value::interval(Int::from(1i64), Int::from(9i64), ctx);
    (1..=2)
        .map(|i| {
            let d = random_element(&digits, ctx, &mut s).unwrap();
            let d = d.as_int().and_then(|v| v.to_i64()).unwrap();
            (x * 3 + d * i) % 61
        })
        .collect()
}

/// Explores the graph with `workers` threads; returns the edges found.
fn explore(seed: i64, workers: usize, scramble: u64) -> BTreeSet<(i64, i64)> {
    let streams = Streams::new(seed, RandomMode::Dive);
    let mut start = streams.startup();
    let ctx = Ctx::dive();
    let range = Value::interval(Int::from(0i64), Int::from(60i64), ctx);
    let mut frontier: Vec<i64> = (0..2)
        .map(|_| {
            random_element(&range, ctx, &mut start)
                .unwrap()
                .as_int()
                .and_then(|v| v.to_i64())
                .unwrap()
        })
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    let mut visited: BTreeSet<i64> = frontier.iter().copied().collect();
    let mut edges = BTreeSet::new();
    while !frontier.is_empty() {
        let chunk = frontier.len().div_ceil(workers);
        let found: Vec<Vec<(i64, i64)>> = std::thread::scope(|scope| {
            let handles: Vec<_> = frontier
                .chunks(chunk)
                .map(|part| {
                    scope.spawn(move || {
                        let mut part = part.to_vec();
                        // A different, arbitrary processing order per run.
                        part.sort_by_key(|x| (x.unsigned_abs() ^ scramble).wrapping_mul(0x9e37));
                        part.iter()
                            .flat_map(|&x| successors(streams, x).into_iter().map(move |y| (x, y)))
                            .collect::<Vec<_>>()
                    })
                })
                .collect();
            handles.into_iter().map(|h| h.join().unwrap()).collect()
        });
        let mut next = BTreeSet::new();
        for (x, y) in found.into_iter().flatten() {
            edges.insert((x, y));
            if visited.insert(y) {
                next.insert(y);
            }
        }
        frontier = next.into_iter().collect();
    }
    edges
}

#[test]
fn replays_at_any_worker_count() {
    for seed in [0, 7, -3, i64::MAX] {
        let reference = explore(seed, 1, 0);
        assert!(!reference.is_empty());
        for workers in 2..=8 {
            for scramble in [1, 0xdead_beef, u64::MAX] {
                assert_eq!(
                    explore(seed, workers, scramble),
                    reference,
                    "seed {seed}, {workers} workers"
                );
            }
        }
    }
}

#[test]
fn different_seeds_draw_differently() {
    assert_ne!(explore(0, 1, 0), explore(1, 1, 0));
}

#[test]
fn a_state_stream_depends_only_on_seed_state_and_call_index() {
    let streams = Streams::new(42, RandomMode::Dive);
    // Draw from one state's stream, interleaved with draws from other states' streams
    // in any order: each stream's sequence is unchanged.
    let mut a = streams.state(1);
    let mut b = streams.state(2);
    let alone: Vec<i64> = {
        let mut s = streams.state(1);
        (0..8).map(|_| s.next_long()).collect()
    };
    let mut mixed = Vec::new();
    for _ in 0..8 {
        let _ = b.next_long();
        mixed.push(a.next_long());
        let _ = streams.state(3).next_double();
    }
    assert_eq!(mixed, alone);
}

#[test]
fn simulation_traces_are_keyed_by_number() {
    let streams = Streams::new(5, RandomMode::Dive);
    let draws = |t: u64| {
        let mut s = streams.simulation_trace(t);
        (0..4)
            .map(|_| s.next_double().to_bits())
            .collect::<Vec<_>>()
    };
    // Whatever order traces run in, trace t draws the same values.
    let forward: Vec<_> = (0..16).map(draws).collect();
    let mut backward: Vec<_> = (0..16).rev().map(draws).collect();
    backward.reverse();
    assert_eq!(forward, backward);
    assert_ne!(forward[0], forward[1]);
}
