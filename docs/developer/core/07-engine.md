# Exploration engine

Status: M0 spike implemented in `crates/dive-explore` on `core/m0-spike`;
results in `m0-results.md`. This document covers strategies S3 and S9 from
`01-motivation.md`. The new engine lives in `dive-explore`.

## 1. Contract

The determinism contract is the one the current engine already meets
(ADR 0015; `crates/dive-check/src/parallel.rs:1-48`).

- **1 worker.** Distinct and generated counts, depth, the violated property,
  the error class and the trace match TLC under `--compat=tlc`, and match the
  goldens in the default profile.
- **N workers.** The result equals the 1-worker result (D-001), for every
  worker count and seed.
- **Event order.** Events are ordered by position, the (state, action,
  ordinal) key of ADR 0015, together with the event phase. The reported event
  is the minimum.

## 2. Data

**State.** An array of `n` persistent handles, one per variable, plus a sparse
lazy-view side table (`03-value-model.md` §8). It is 8n bytes, with no
pointers to chase.

**Frontier.** The states of the current level, in position order:
- a flat vector of handle arrays;
- a parallel vector of state ids for the predecessor log.

The frontier can spill through the existing frontier code in `dive-store`
(`crates/dive-store/src/spill.rs:66`), adapted to handle arrays.

**Seen set.** An open-addressing table of 64-bit state hashes.
- Each slot stores the hash and a claim: the minimum position that produced
  that hash in the current level.
- Claims use compare-and-swap on a packed 64-bit position word, so no lock is
  needed. This is safe Rust using `AtomicU64`.
- **Collision reporting.** It is computed as `crates/dive-store/src/fpset.rs`
  does today.
- **Growth.** The table grows at level boundaries. Within a level, sizing
  uses the level's upper bound or a resize protocol, decided in M1.

**Exact store.** `--store exact` keeps full canonical keys, via the existing
`dive_store::ExactSet` or a handle-array variant of it.

**Predecessor log.** `dive_store::PredecessorLog`
(`crates/dive-store/src/predecessor.rs:57`) records parent id and successor
ordinal, unchanged.

## 3. One level

**Expand.**
- Workers claim batches of work units from an atomic counter. On narrow levels
  a work unit is one (state, action) pair, as in the current engine
  (`parallel.rs:5-10`).
- For each unit, the worker enumerates successors with the compiled
  generator (`05-successor-generation.md`) into its reusable successor buffer.
- For each successor it then:
  1. updates the generated count;
  2. computes the state hash from the parent hash and the delta (S1);
  3. probes the seen set.
- **Already present from earlier levels:** discard. Nothing was built (S3).
- **New, or claimed in this level:** try to lower the claim to this position.
  - On failure, discard.
  - On success, materialize the successor: copy its handle array into the
    worker's candidate list, interning transient components (deep close) only
    at this point.
  - A later, lower claim makes an earlier candidate redundant. The gather step
    drops it.

**Gather.** After a barrier:
- each worker keeps only candidates whose recorded position still equals the
  slot's claim;
- the surviving candidates are merged in position order into the next
  frontier, with a parallel merge of per-worker sorted runs.

**Check.**
- Invariants are evaluated in parallel on the next frontier's new states, with
  memoization (`06-memoization.md`).
- Violations and errors are recorded with their positions, and the minimum
  event decides the stop.
- Counts at a stop are computed as ADR 0015 §3 defines.

**Commit.**
- The predecessor log receives the new states in position order.
- Slot claims are cleared, so their hashes become plain "present" entries.

The barriers per level are expand→gather and check→commit. On narrow levels
the cost of a barrier is bounded by splitting levels into (state, action) units.
On very wide levels it is amortized.

## 4. Errors, traces and replay

An evaluation error at a position is an event like a violation. The minimum
event wins.

The trace is rebuilt from the predecessor log by replaying the recorded
ordinals with the new core's generator, as `crates/dive-check/src/engine.rs`
`replay` does now. The replay runs with memoization off. The call-stack replay
that classifies certain errors follows `02-semantic-contract.md`; its effect on
counts is reproduced.

Every counterexample is emitted in the dive-trace format. In both profiles it
is validated by `dive-trace`, which runs on the reference evaluator
(`08-correctness.md`).

## 5. Parallelism without contention (S9)

The design avoids these, by construction:

- **Process-global locks on the hot path.** Host hooks, the reachability of
  effects and the profile are resolved when the plan is built. Issue #495 is
  the motivating case.
- **Shared reference counts.** Values are handles, so there are no reference
  counts at all.
- **Shared allocator traffic.** Transients live in per-worker arenas, and
  candidate lists are per-worker vectors that are reused.
- **Contended writes on the shared store.** Interning happens only for claim
  winners, under per-shard locks spread across 256 shards.
- **Serial coordinator work.** Gather, check and commit are parallel. Only the
  O(workers) bookkeeping is serial.

The target is at least 6× scaling at 8 workers on wide-level models. It is
measured per model (`09-performance-method.md`).

## 6. Runtime names under `--compat=tlc`

Under TLC order, strings created at runtime (`ToString`, `\o`, `SubSeq`,
`Tail` on strings, and others) get intern tokens in encounter order. Those
tokens affect comparisons, set order and `CHOOSE`
(`02-semantic-contract.md`). With several workers the encounter order is
nondeterministic.

The reference engine handles this by forcing a single worker
(`crates/dive-model/src/reach.rs:248`;
`crates/dive-check/src/engine.rs:2247`).

- **M1.** Same rule: the plan records whether runtime names are reachable
  under TLC order, and if so the run uses one worker.
- **M2.** A worker that creates a new string during a level gives it a
  provisional token, ordered by (position, sequence within the evaluation).
  This is the order a single worker would have produced.
  - **Comparisons.** Any comparison or sort that involves a provisional token
    sets a per-level flag. Memoization is off for those inputs
    (`06-memoization.md` §4).
  - **Barrier, no comparison.** If the flag is clear, provisional tokens are
    made final in position order.
  - **Barrier, comparison made.** If the flag is set, the level is
    re-executed on one worker, with exact tokens.
  - **Expected cost.** Most levels after warm-up create no new strings, so
    re-execution should be rare. This is measured.

## 7. Liveness, simulation and other modes

**Liveness.**
- In M3 the engine gains a graph-capture mode. It records states and edges
  with labels, as `dive_check::capture::BehaviorGraph` does
  (`crates/dive-check/src/capture.rs:253`).
- Captured states are converted to `dive_value::Value` arrays by an adapter,
  so that the existing `dive-liveness` tableau and search work unchanged.
- A native port of the liveness atoms to the new evaluator follows only if the
  adapter is measured to be a bottleneck.

**Other modes.**
- Simulation reuses the compiled generator with a random choice of successor.
- DFID, checkpoints, budgets, spill and the disk seen set are ported in M3
  from their current designs (ADRs 0021, 0033, 0107, 0024, 0063).
- Until then, these modes use the reference engine through the static
  fallback.

## 8. Next step

M0 built a single-thread breadth-first search. It follows TLC's
`ModelChecker` order of counting, constraint, seen and invariant checks. It
uses an open-addressing seen set of 64-bit state keys, and probes each
successor with the incremental hash before any interning. Only new states are
interned. `dive check --engine=core` (or `DIVE_ENGINE=core`) reaches it
through `crates/dive-api/src/core_engine.rs`. It honours `--time-limit`.

Not built in M0: the predecessor log, traces, error classification,
`--max-memory` enforcement inside the core, and parallelism.

Next (M1, stream 3):
1. Add the predecessor log and traces through `dive-trace`.
2. Build the parallel level of section 3 on the store measured in
   `m0-results.md`.
3. Add budgets and the static fallback (`08-correctness.md`).
