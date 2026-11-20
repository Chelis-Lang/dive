# 0159: Keep a scoped worker pool for temporal capture

- **Status:** accepted
- **Date:** 2026-11-19

## Context

Temporal graph capture limits each wave to a small number of source rows.
Starting OS threads at every wave can cost more than expanding those rows.
In a CoffeeCan1000Beans ablation, retaining the pool brought the eight-worker
check from 10.75 to 5.65 seconds. Reusing only the claim table took 10.29
seconds. At 3,000 beans, the eight-worker check went from 107.3 s to 49.4 s.
The one-worker times were 50.2 s and 48.8 s. These are engineering runs on
a shared Ryzen AI MAX+ 395 workstation, with a locked release build, LTO
disabled, 16 codegen units, and process affinity to eight CPUs. The original
release D4 survey used a separate host.

## Decision

Use a run-local `rayon-core` pool in `dive-explore` for multi-worker temporal
capture. A scoped task borrows one machine and one result slot per worker.
Join the scope before the coordinating thread commits in discovery order.
Keep the claim table private to the search and reset it after each join.

## Consequences

The pool can borrow the current frontier and evaluator state under Rust's
ordinary lifetime rules. The checker still controls worker count, capture
limits, cancellation and ordered error reporting. The dependency brings
Crossbeam's work queues into the build. Graph commits and temporal analysis
still limit attainable parallel speedup.

The selected width still describes parallel expansion. An interleaved
PConProof control that extended the pool to ordinary safety checking was
slower (99–109 s versus 84–101 s at eight workers), so ordinary safety
retains its existing worker lifetime. [Original D4 results](../developer/release-performance-results.md) · [Issue #1028](https://github.com/Chelis-Lang/dive/issues/1028).
