//! The Examples golden of `SpanningTree/SpanTreeRandom` (made with `-seed 0`), checked
//! by hand: TLC draws the random graph `Edges` from the start-up stream, so its state
//! count is a function of the first six draws of `java.util.Random(0)`.
//!
//! ```tla
//! Edges == UNION { {{n, m} : m \in RandomElement(SUBSET (Nodes \ {n}))} : n \in Nodes }
//! ```
//!
//! `Edges` is a zero-arity constant definition, so TLC evaluates it once at start-up
//! (`SpecProcessor.processConstantDefns`), before the `ASSUME`s, with one
//! `RandomElement` per node in the normalized order of `Nodes`, each over the subsets
//! of the other nodes in TLC's set order. The next-state relation is then
//! deterministic. The golden records 79,550 distinct states and 1,357,252 states
//! generated (`conformance/golden/tlc-2026.10.06.014338/examples/specifications/
//! SpanningTree/SpanTreeRandom.cfg.json`).

#![allow(
    clippy::unwrap_used,
    clippy::many_single_char_names,
    reason = "test code; short names mirror the TLA+ specs"
)]

use std::collections::HashSet;

use dive_stdlib::random::{RandomMode, Streams, random_element};
use dive_value::{Ctx, Interner, Value};

const N: usize = 6;
const MAX_CARDINALITY: u8 = 6;

/// The adjacency matrix of TLC's `Edges` for `seed`.
fn edges(seed: i64) -> [[bool; N]; N] {
    let ctx = Ctx::tlc();
    let names = Interner::new();
    // The model values of the cfg, `Nodes = {n1, ..., n6}`, created in this order.
    let nodes: Vec<Value> = (1..=N)
        .map(|i| Value::model(&names, &format!("n{i}")))
        .collect();
    let index = |v: &Value| nodes.iter().position(|n| n == v).unwrap();
    let all = Value::set(nodes.clone(), ctx).unwrap();
    let mut s = Streams::new(seed, RandomMode::Tlc).startup();
    let mut adj = [[false; N]; N];
    for n in all.as_set().unwrap().elements(ctx).unwrap().iter() {
        let others: Vec<Value> = nodes.iter().filter(|m| *m != n).cloned().collect();
        let subsets = Value::subset(Value::set(others, ctx).unwrap(), ctx);
        let chosen = random_element(&subsets, ctx, &mut s).unwrap();
        for m in chosen.as_set().unwrap().elements(ctx).unwrap().iter() {
            let (a, b) = (index(n), index(m));
            adj[a][b] = true;
            adj[b][a] = true;
        }
    }
    adj
}

/// Breadth-first search of `SpanTreeRandom` with `Root = n1`: (distinct, generated).
fn explore(adj: &[[bool; N]; N]) -> (usize, usize) {
    type State = ([u8; N], [u8; N]); // (mom, dist)
    let mut mom = [0u8; N];
    for (i, m) in mom.iter_mut().enumerate() {
        *m = u8::try_from(i).unwrap();
    }
    let mut dist = [MAX_CARDINALITY; N];
    dist[0] = 0;
    let init: State = (mom, dist);
    let mut seen: HashSet<State> = HashSet::from([init]);
    let mut frontier = vec![init];
    let mut generated = 1;
    while !frontier.is_empty() {
        let mut next = Vec::new();
        for (mom, dist) in frontier {
            for n in 0..N {
                for m in (0..N).filter(|&m| adj[n][m]) {
                    // `dist[m] < 1 + dist[n]` in the spec.
                    if dist[m] > dist[n] {
                        continue;
                    }
                    for d in dist[m] + 1..dist[n] {
                        let (mut mom2, mut dist2) = (mom, dist);
                        dist2[n] = d;
                        mom2[n] = u8::try_from(m).unwrap();
                        generated += 1;
                        if seen.insert((mom2, dist2)) {
                            next.push((mom2, dist2));
                        }
                    }
                }
            }
        }
        frontier = next;
    }
    (seen.len(), generated)
}

#[test]
fn span_tree_random_seed_0_matches_the_golden() {
    let (distinct, generated) = explore(&edges(0));
    assert_eq!((distinct, generated), (79_550, 1_357_252));
}
