# Exploration engine

Status: M1 stream 3. Single-thread BFS in `crates/dive-explore`, with the
predecessor log and replayed traces (§4, `core/engine-traces`) and TLC's
error endings (§4, `core/engine-errors`). The parallel level (§3, S9) is not
built yet. This document covers strategies S3 and S9 from
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

**Done (M0).** A single-thread breadth-first search in TLC's `ModelChecker`
order of counting, constraint, seen and invariant checks. It uses an
open-addressing seen set of 64-bit state keys and probes each successor with
the incremental hash before any interning, so only new states are interned.
`dive check --engine=core` (or `DIVE_ENGINE=core`) reaches it through
`crates/dive-api/src/core_engine.rs`. It honours `--time-limit`.

**Done (M1, `core/engine-traces`).**
- Every enqueued state is logged in a `dive_store::PredecessorLog` as
  `(parent, ordinal)`. The ordinal is the successor's index among all
  emissions of its parent's expansion, across actions in order, or its index
  among the initial states. The frontier carries the log ids beside the
  handle arrays. The log costs 16 bytes per distinct state: MCLamportMutex
  peaks at 92–97 MB instead of 73 MB (M3, indicative, load 10).
- At a stop, the explorer records where the behavior ends: the parent and the
  violating successor (interned from the buffer before the arena is reset),
  or the deadlocked state. `dive_explore::trace::replay` regenerates the path
  with memoization off and checks that it reproduces the state the search
  held. A mismatch is an internal error, never a trace.
- `core_engine.rs` converts the handles back to reference values. It builds
  the reference engine's `Counterexample`, with the same labels and action
  names, so the report and its `dive-trace` document come from the same
  code (`dive check --output json`, `counterexample.trace`).
- `crates/dive-api/tests/core_traces.rs` runs the rule fixtures, the TLC
  probes and the core fixtures. For each one, the core's trace document
  equals the reference engine's (both are minimal-depth BFS traces over the
  same successor order), and `dive-trace` validates it. For deadlocks the
  validator checks the steps and reports the ending itself as unsupported.

**Done (M1, `core/engine-errors`).** Error endings
(`02-semantic-contract.md` §3(e)).
- `dive_explore::ending` records where an evaluation error ends the search:
  - the site: `Init`, `Next(action)`, `Constraint(i)`, `ActionConstraint(i)`,
    `Invariant(i)`, or `Incomplete` for a state that leaves variables
    unassigned;
  - the parent state, the state checked, and the action;
  - whether the state was inside the model;
  - for `Init` and `Next`, the states the predicate or action emitted before
    the error.
  The trace ends at the parent, followed by the state checked.
- TLC's call-stack replay (E3) is the explorer's. After an invariant fails
  to evaluate on a new successor inside the model, the parent is expanded
  again, each action in full before its successors are checked. Past the
  failing successor, new states are inserted (distinct, not generated),
  checked and queued, until a violation or an incomplete state ends the
  replay quietly, or a further error ends it with that error's site.
- The core's errors carry no TLC class yet (`04-plan-compiler.md` §5, stream
  2). `crates/dive-api/src/core_ending.rs` therefore reproduces each ending
  with the reference evaluator on the same states: the failing action is
  re-run (`next_states` or `init_states`), or the failing check is evaluated
  again. The error it raises is classified with the reference engine's table,
  through `dive_check::classify` (a thin public view of `class.rs`):
  - E2 for `Next` and the constraints;
  - E3 for invariants, with `recurring` for a state outside the model and
    `replay_error` for a further error in the replay;
  - E6 for the initial states, with `init_error` and TLC's statistics after
    an error there.
  The reproduction doubles as a cross-check. If the reference evaluator
  raises no error there, or emits a different number of states before it,
  the run is refused rather than reported.
- When the core has typed error provenance, the reproduction can be
  dropped: build the `EvalError` from the core error's EC code and kind, and
  call the same table.
- Results, in `crates/dive-api/tests/core_rules.rs` and `core_traces.rs`:
  - Of the 175 error-class fixtures, 85 agree with the reference engine on
    verdict, class, exit, counts after the replay, and trace. Among them,
    `ErInvDivZero` gives 4 generated, 5 distinct, 2 queued and exit 76, and
    `ErInvReplayError` gives 4 / 5 / 1 and exit 75.
  - The other 90 use constructs the core does not run yet (VIEW, ALIAS,
    POSTCONDITION, properties, `Print`, `Assert`, ENABLED, failing ASSUMEs,
    recursive functions). `ErFpInfinite` is the one exception: it is refused
    because the core raises its error while assigning `x' = Nat`, where TLC
    raises it only when fingerprinting the successor.
  - Every trace equals the reference engine's and validates with
    `dive-trace`. The validator's known refutations #490 and #499 are
    allowed, as for the reference engine.

**Not built yet:** fingerprint-site errors, budgets other than
`--time-limit`, and the parallel level.

**Next (M1, stream 3):**
1. Budgets: `--max-memory`, `--max-states` and `--time-limit` with the
   reference engine's structured stop.
2. The parallel level of §3.
3. The static fallback (`08-correctness.md` §3).
