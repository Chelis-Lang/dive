# Exploration engine

Status: M1 stream 3. BFS in `crates/dive-explore`, with the predecessor log
and replayed traces (§4, `core/engine-traces`), TLC's error endings (§4,
`core/engine-errors`), budgets (`core/engine-budgets`), the parallel level
(§3, `core/engine-parallel`) and `--engine=auto` (`core/engine-auto`,
`08-correctness.md` §3). This document covers strategies S3 and S9 from
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

The opt-in safety frontier spills through `dive-store::spill::OrderedFrontier`.
It appends paired canonical/original handle rows directly and decodes at most
1,024 pairs per expansion chunk. A narrow `dive-core` codec owns the live Store
and a private nonreused process token; original/history views remain exact.
This is same-process transient storage, not a value snapshot or recovery format.

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

**Memory (#551).** As implemented, a level is expanded and committed a chunk
of states at a time (4,096 per worker), so phase 1 buffers one chunk's
successors, not the level's. A chunk's commit fills the seen set before the
next chunk is expanded, and reads it as an earlier level's: positions only
matter within a chunk. Transients are freed by arena rollback, under the
rule of `03-value-model.md` §11.5 (no thunk forced to a transient since the
mark): after each accepted state's checks, and after each element of an
`x \in S` or filtered membership once its continuation has emitted. Before
this, MCBakery (655,200 initial states, depth 1) held every candidate's
values until the level ended: 769 MB against the reference engine's 216 MB
at 1 worker. After it, 136 MB at 1 worker and 174 MB at 8 (local, under
load). Fixtures `WideLevel` and `WideRepeat` (levels wider than a chunk,
with states repeated across chunks) agree with the reference engine at 1,
2, 3 and 8 workers.

## 4. Errors, traces and replay

**Initial depth (M1 closeout).** After successful initial-state generation,
depth is at least one if any candidate was emitted, even when constraints
exclude every candidate. A truly empty Init keeps depth zero. Pinned TLC BFS
(`tlc-2026.10.06.014338`, one worker, seed/fp 0) confirms
`SimExcludedInitial` has 1 generated, 0 distinct and depth 1; the three-state
`ExcludedInitial` control has 3 / 0 / 1 and `EmptyInitial` has 0 / 0 / 0.
`core_initial_depth` checks both profiles, one, four and eight requested workers,
and reference/core/auto routing. Init failures retain their existing
stop-specific accounting.

**No next-state action (M1 closeout).** After successful Init, a nonempty
frontier with no bound next-state action ends with TLC's 2115 model error,
before any state expansion. This is independent of the deadlock setting.
Pinned TLC BFS (`tlc-2026.10.06.014338`, one worker, seed/fp 0) confirms
`DfidInitialOnly` exits 75 without a trace or printed statistics. An empty
Init succeeds; a defined but disabled action instead deadlocks unless
deadlock checking is disabled. An invariant with no configured Next still
fails earlier at binding (2232, exit 151). `core_no_next` checks these
boundaries in both profiles at one, four and eight requested workers.
The combined `ExcludedNoNext` control emits one initial state, excludes it
by constraint, and defines no Next. Pinned TLC succeeds with 1 generated,
0 distinct and depth 1, with deadlock checking on or off; both regression
suites retain this interaction.

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
  the motivating case. String comparisons in the canonical order read each
  worker's heap copy of the string table's texts, extended on a miss, not
  the table behind its lock (#550: in SimpleRegular, `Strings::cmp` was
  the hottest frame after `Heap::view` at 8 workers, and the eleventh at
  1 worker; with the copy, user time at 8 workers halved).
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
- **As built (M2).** The core seals its string table after the start-up
  names and refuses any later string (`core/engine-compat`), so no worker
  ever creates a token and compat runs at any worker count. The provisional
  tokens below are needed only to run such models instead of refusing them.
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

**Implied properties first (M1).** Most `PROPERTY` entries in the bench
and the corpus hold no genuine temporal obligation. The binder splits each
property into TLC's roles (`dive_model::Model`): `[]P` joins the
invariants, a state-level part becomes an implied initial predicate
(`implied_inits`), and `[][A]_v` becomes an implied action
(`implied_actions`). Only what is left is a temporal property for
`dive-liveness` (`implied_temporals`). The reference engine checks the first
three in its safety search (`crates/dive-check/src/engine.rs`,
`check_implied_actions`), and reports `"check_kind": "safety"` for such a
model. Both bench models the lead routed to the liveness adapter are of this
kind: EWD998PCal (`PROPERTIES EWD998Spec`, a refinement whose only roles are
implied initial predicates and actions) and BufferedRandomAccessFile (nine
properties, all `[]P` or `[][A]_v`). Their time is spent in those checks,
not in a tableau: on an 8-core laptop under load, the reference engine takes
9.4 s for EWD998PCal without its property and 85 s with it. The core needs
no graph or adapter for them, only the two roles in its own search:
- Plan: `implied_inits` are state predicates and `implied_actions` step
  predicates, compiled as `INVARIANT`s and `ACTION_CONSTRAINT`s are.
- Order, as the reference engine's `Search::successor`: on a new initial
  state, the implied initial predicates come after the invariants, and a
  false one is `PropertyViolatedInitially`. On every step, new or seen,
  inside the model or not, the implied actions come after the new state's
  invariants. A false one is `ActionPropertyViolated`; the trace ends with
  the successor.
- An evaluation error in either is reported at `ErrorSite::ImpliedInit` or
  `ErrorSite::ImpliedAction`, reproduced by the reference evaluator as other
  check errors are.
- A model that also has a genuine temporal property still falls back. So do
  TLC's replay after an invariant error and SYMMETRY, until each is shown to
  agree.

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
    recursive functions).
  - Every trace equals the reference engine's and validates with
    `dive-trace`. The validator's known refutations #490 and #499 are
    allowed, as for the reference engine.

**Done (M1, `core/engine-budgets`).** Budgets (ADR 0107).
- The run's armed budget (`--time-limit`, `--max-memory`; the watchdog is
  `dive-api`'s, shared with the reference engine) is polled where the
  reference engine polls it: before each state is expanded, and before each
  initial state. A poll is one relaxed atomic load.
- `--max-states` and `--max-depth` are admission limits. A new state inside
  the model that would exceed one stops the search before it is inserted,
  as the reference engine's `admission_stop` does. A state already seen is
  never refused.
- The stop is the reference engine's structured `stopped` verdict: the
  resource, limit and observation, the phase `safety-search`, the
  statistics at the stop, and the exit code. TLC's statistics are not
  printed for a stop in the initial states. The report's `options` and
  `resources` sections, which `dive-bench` reads, are unchanged.
- `crates/dive-api/tests/core_budgets.rs`:
  - every probe and core fixture runs under six admission limits, and the
    report equals the reference engine's;
  - the time and memory limits each stop an endless model.

**Done (M1, `core/engine-parallel`).** The parallel level, implemented in
`crates/dive-explore/src/parallel.rs`. With `--workers N`, the levels after
the initial states run in two phases:
- **Expand** (parallel). Workers claim batches of states from an atomic
  counter. Each worker has its own `Machine` and shares the store. For each
  successor, a worker evaluates completeness, the constraints, the key and,
  unless the state is a duplicate, the invariants. It interns only the
  successors that may be new.

  Duplicates are found by lock-free minimum-position claims: an `AtomicU64`
  key table with `fetch_min` on a packed (state, emission) position. A
  successor inside the model whose key is held by an earlier position is not
  new, and is neither interned nor checked. A claim only saves work: a key
  that is not placed within 32 probes (the table is at most 2^23 slots), or
  a position past 2^24 successors of one state, is checked anyway.
- **Commit** (sequential, in position order). The explorer replays the
  one-worker decisions over the recorded successors, without evaluating
  anything: counting, admission limits, seen-set insertion (the first
  position wins), the first event, queueing, the predecessor log, and
  warnings in the order the one-worker search raises them. Error endings,
  TLC's call-stack replay and traces reuse the one-worker code.

Every observable result is therefore the one-worker result:
`crates/dive-api/tests/core_parallel.rs` checks every fixture the core runs
at 2, 3 and 8 workers, with and without `--max-states`, and
`core_golden.rs` checks the spike models at 4 workers.

Scaling is not measured yet, because the M3 development machine was busy
with other work (load averages 12–19 on 4 performance and 4 efficiency
cores). Indicative figures, explore time at 1 / 4 / 8 workers:
- MCLamportMutex: 9.1 / 4.3 / 3.2 s.
- GameOfLife: 7.5 / 2.2 / 1.7 s.

The commit phase takes about 0.2 s of MCLamportMutex's run (`commit_ms` in
the core metrics), so it is not the limit. The ≥ 6× target at 8 workers
needs a quiet x86 machine to assess (`09-performance-method.md`).

**Native safety storage.** `--store exact` retains complete canonical rows in
hash buckets and resolves collisions using normalized mathematical equality.
VIEW and symmetry use the identity produced by their existing single key
evaluation. Receiver/history rows remain separate. Parallel fingerprint claims
are disabled in exact mode: a claim cannot prove full-state equality. Ordered
commit still decides duplicate admission, limits and the first error.

The existing fallible `PredecessorLog` now takes the caller's resident byte
limit and directory. It validates and explicitly closes before the explorer
publishes a result. Typed storage failures retain prefix counts and component
evidence, clear certified words and produce a resource-limit result. This is
neither a whole-process memory bound nor a checkpoint format.

The frontier policy uses the existing per-queue encoded capacity and private
checksummed files. Queue order, concrete discovery IDs and canonical/original
roles survive spill. Explicit close validates even unvisited retained records
before certification, polling the actual owner every 128 rows; an already
stopped search cleans up without scanning those rows. Decoding also polls that
owner and cancellation remains a budget stop. Codec/read/write/corruption/cleanup
failures clear certification and remain explicit component evidence. Reported
retention covers encoded capacities observed at queue operations, excluding
Store/history, decoded chunks, candidates, seen sets, arenas and allocator work.
Disk membership, native checkpoints and temporal exact/spill remain refused.

**Done (M1, `core/engine-auto`).** `--engine=auto`, the static and dynamic
fallback of `08-correctness.md` §3. With it, run options the core does not
implement (TLC fingerprints outside compatibility, non-default evaluation
options, disk-seen spilling, `--chaos`) are static refusals. `--engine=core`
used to ignore them.

**Done (M1, `core/engine-fperror`).** Fingerprint-site errors. TLC
fingerprints a new state inside the model before inserting it, and fails on
a value holding `Nat`, `Int`, an infinite `Seq(S)`, or an infinite set whose
fingerprint enumerates one (`SUBSET Nat`, `[Nat -> S]`, record sets and
products). States outside the model are not fingerprinted.
- The core checks the same new states, after the constraints and before the
  insertion, in both the one-worker and the parallel level. It interns the
  variables that differ from the parent's, which closes every finite set, so
  any set kind left is infinite; such a state ends the run at
  `Site::Fingerprint`.
- The ending converts the state to the reference engine's values (infinite
  sets as its lazy sets) and fingerprints it with `dive-store`'s
  `StateFingerprinter`. That error is classified at `ErrorSite::Fingerprint`,
  as the reference engine does, and the trace ends at the predecessor. If
  the reference fingerprints the state, the run is refused, never reported.
- `ErFpInfinite` now agrees (85 error fixtures). Fixtures `Fp*` in
  `crates/dive-api/tests/fixtures/core-m1` cover `Int` in a tuple,
  `Seq({1})` in a record, `SUBSET Nat`, a record set, an initial state and a
  state outside the model, at 1, 2, 3 and 8 workers (`core_parallel.rs`).
- Still refused: a state holding `{Nat, 3}`, whose TLC error depends on
  when TLC normalizes the set (C1, C2), and lazy functions over `Nat`
  (`ErFpLambda`, which the reference engine refuses too).

**Done (M1, `core/engine-order-note`).** A stop in the default profile (any
verdict about the model other than success) carries the reference
engine's D-010 note, so the core's text output equals the reference
engine's on violations, deadlocks and error endings. Under `--compat=tlc`
there is no note. `core_rules.rs` compares the notes on every fixture.

**Done (M2, `core/engine-compat`).** `--engine=core --compat=tlc`, on
one worker. The run's evaluation options map to `dive_core::Semantics`,
which the store holds for its lifetime and every heap reads, so constant
folding, the search and trace replay all compute alike. Each component has
its own switch:
- **String order** (`StringOrder::Token`, D1). The table is preloaded with
  the reference binder's start-up names (`Model::names`, tokens below
  `Evaluator::startup_names`) and then sealed. A string first met later,
  during compilation or the search, is refused, so §6's runtime-name rule
  holds without forcing anything: such a run never reaches the search.
- **Integers** (`IntMode::Tlc`). The heaps compute in Java `int`, and
  `\div` from `Naturals` alone compiles to `IntOp::DivNaturals`. A numeral
  outside 32 bits is refused at compile time. The reference binder already
  fails on it (`TLC_INTEGER_TOO_BIG`), and the core refuses that binding
  failure.
- **Comparisons** (C1–C5). `compare.rs` keeps the D-009 answers and adds a
  shape guard. Under TLC's comparisons, an equality between unequal compound
  values and a membership miss of a compound value must have shapes that unify.
  In both profiles, the elements of a new compound set must also unify: the
  same class at every position TLC's
  `equals` or `compareTo` could inspect, with records and tuples kept apart.
  A bounded allocation-free proof handles closed integer-domain functions
  with an identical domain and matching Bool/inline-Int/owned-string classes
  at each key. It inspects every field, including shared handles, within a
  shared 64 function-and-entry budget. These functions have identical complete
  comparison shapes; the original shape guard handles every other case,
  including differing domains, nested sets and raw/history-bearing views.
  When the shapes unify, TLC cannot fail, and its answer is TLA+'s. When
  they do not, the core refuses. Membership of a function in `Seq(S)`, in a
  record set or in a product, where D-009 answers `FALSE` but TLC's `member`
  fails, is refused too. Stream 2's exact port of TLC's checked comparisons
  replaces the guard.
- **Exploration order** (A6). `x' \in S` over a lazily represented set (the
  `Raw` structures) is refused when TLC's `elements()` sequence is not in
  normalized order. Otherwise the normalized order is TLC's order.
- **Fingerprints.** The core keeps its own 64-bit state keys. TLC's
  fingerprint values are an internal (ADR 0086); only collisions could
  differ, and TLC's optimistic estimate is computed from the counts.
- `--engine=auto` first kept `--compat=tlc` on the reference engine. It
  runs it on the core since `core/engine-auto-compat`.

Results:
- `crates/dive-api/tests/core_compat.rs` runs every fixture under
  `--compat=tlc` on both engines and compares verdict, counts, exit,
  output, notes and the counterexample's states. It also checks TLC's
  counts where TLC completes. Agreement: 84 error fixtures, 16 rule
  fixtures, 18 probes, 11 M1 fixtures, and the five `core-compat` fixtures,
  of which `CompatShape` is refused by rule. `CompatStrOrder` shows that
  the string order is observable.
- Shadow, `--impl-profile tlc`: the 26 smoke models agree, and so do 49
  small-tier models (`08-correctness.md` §4).
- Against the TLC-profile goldens (`dive-conformance run --impl dive:core
  --impl-profile tlc`), all 26 smoke models agree, every one on the core.

**Done (M2, `core/engine-bindfail`).** A model that fails to bind, in
either profile, ends the run as on the reference engine. The core binds
without the run's host, so `core_engine::binding_failure` confirms the
failure with the reference engine's own binding (`dive_check::check`,
which stops at the same point) and reports that result, printed output and
warnings included, when the verdict is the same. Otherwise it refuses. The
fixture `CompatBigNumeral` (a numeral outside 32 bits) is a model error
under `--compat=tlc` and a success in the default profile, on both engines.

**Done (M2, `core/engine-compat-parallel`).** Compat runs at any worker
count. The sealed string table means no worker creates a token, and the
commit phase gives the one-worker result as in the default profile.
- `core_compat.rs` checks every fixture the core runs (134) at 2, 3 and 8
  workers against its one-worker result.
- Compat shadow, small tier at 1, 4 and 8 workers: 50 models agree at every
  worker count, 106 are refused, 0 findings. The sample covered 2,451
  states with no mismatch.

**Done (M2, `core/engine-auto-compat`).** `--engine=auto` runs
`--compat=tlc` on the core, with the usual static and dynamic fallback.
Against the TLC-profile goldens on the small tier (examples, probes,
validation-smoke, configured-temporal-operators; 156 models at 1 and 4
workers):
- `--impl dive:auto`: 150 agree and 6 unsupported. That is the reference
  engine's own result (`--impl dive:reference`: 150 and 6), with 0
  disagreements. 48 models ran on the core, 100 fell back statically and 8
  ended before any search.
- `--impl dive:core`: 50 agree and 106 unsupported, with 0 disagreements.

**Done (M2, `core/engine-implied`).** Implied initial predicates and
actions of `PROPERTY` entries (§7, "Implied properties first"). The plan
compiles them as checks and step predicates; the one-worker search, the
parallel commit (an `Inv` verdict per successor, applied whether or not the
successor is new) and TLC's call-stack replay check them in the reference
engine's order. The 13 models the core refused for "temporal properties"
now run on it: on the small tier under `--impl dive:auto` the core runs 65
models in the default profile and 64 under `--compat=tlc` (48 each
before), with no regression against either baseline. EWD998PCal runs on the
core in 20 s against the reference engine's 80 s and TLC's 35 s (1 worker,
an 8-core laptop under load, its `ALIAS` removed). BufferedRandomAccessFile
still needs `ENABLED` (stream 2) and `SYMMETRY`. Fixtures `PropRefine`
(refinement through INSTANCE, steps out of the model), `PropStep`,
`PropInit` and `PropAbs`; the `ErProp*` error fixtures and the
`ImpliedActionSeen` probe agree. The `ErPropInit*` fixtures need
non-constant split arguments (stream 2), and are refused naming them.

**Done (M2, `core/engine-symmetry`).** `SYMMETRY` in the default profile.
- `core_engine.rs` prepares the declaration as the reference engine does:
  `Model::prepare_symmetry` in `Mode::Checked`, with its warnings and the
  D-010 symmetry note. Then it hands the closure's nonidentity maps to the
  plan, by model-value name. `dive-model`'s `Reduction` gained two
  accessors for this, `universe` and `maps`.
- `dive_core::symmetry::Symmetry::canonical` computes a state's canonical
  image: the least of its images, the state itself first, compared variable
  by variable with `Heap::cmp`. A later image wins only when it is strictly
  smaller, as in `Reduction::canonical_values`. Images are transient heap
  values. A value is rebuilt only when something in it moves.
- The explorer's `state_key` keys a state by its image, in the one-worker
  level, the parallel level and TLC's call-stack replay. The explored state,
  the predecessor log and the traces stay concrete.
- A state outside the model is checked without a lookup, as TLC checks it
  (`isSeenState` is only for states inside the model). Without symmetry a
  lookup only skips a state already checked; with it, a seen image can hide
  a concrete state that was not. Fixture `SymOutside`.
- Refused: `SYMMETRY` with a property that needs liveness checking, and
  states holding lazily represented sets or functions.
- Under `--compat=tlc` (`core/engine-symmetry-compat`), `Symmetry::tlc`
  compares candidates as `TLCStateMut` does and as the reference engine's
  `Reduction::canonical_values` does in TLC's order: TLC's `compareTo`
  (`dive_value::tlc::compare`), variable by variable in
  `Ir::tlc_var_order`. The core's `tlc_cmp` answers where `compareTo` orders
  as `Heap::cmp` does under TLC's string order: values of one class, and
  untyped model values, which come first. It refuses what `compareTo` would
  fail on (a typed model value against another kind of value or type,
  values of two classes) and symbolic sets. No D-010 note is added. For a
  valid symmetry, the choice of image does not change the orbits, so the
  counts are the default profile's; the order matters for the refusals and
  the errors.
- Fixtures `SymOwner` (a violation) and `SymSets` (a deadlock) in
  `core-m1` agree with the reference engine in counts and traces, at every
  worker count `core_parallel.rs` checks.

**Done (M2, `core/engine-alias`).** `ALIAS`. The reference engine does not
apply ALIAS yet: its counterexample shows the states themselves, with a
note saying so. The core now does the same instead of refusing, and its
notes keep the reference engine's order: the ALIAS note, then the D-010
stop note. The ten `ErAlias*` error fixtures now agree, so at least 110 of
the 175 do. Applying ALIAS to traces is reference-engine work, and the core
follows it.

**Done (M2, `core/engine-view`).** `VIEW`. The plan compiles the view as
a one-conjunct value role, and `state_key` keys a state by the key of the
view's interned value. As in TLC's `TLCStateMut.fingerPrint`, the state's
variables are normalized first (the core interns them, and refuses a state
or view value holding an infinite set rather than reproduce TLC's
normalization of it). An evaluation error in the view is
`ErrorSite::View`, reproduced by the reference evaluator; the trace ends at
the predecessor, as for a fingerprint error. A state outside the model is
checked without a lookup, as under SYMMETRY. Fixtures `ViewMerge` (an
invariant that reads a variable the view ignores, violated on a state
outside the model whose view was seen) and `ViewSets`; nine `ErView*`
error fixtures agree (`ErViewEnabled` waits for `ENABLED`).

**Done (M2, `core/engine-view-symmetry`).** VIEW together with SYMMETRY.
TLC evaluates the view on the state's canonical image
(`TLCStateMut.fingerPrint`; the reference engine's `state_key_of`), so
`view_key` computes the image first and evaluates the view with the image
as the current state. The checks, the predecessor log and the traces stay
concrete. Fixtures `SymView` (a view that is not symmetric: one
process's counter) and `SymViewSets` (sets and sequences, to a deadlock)
agree with the reference engine; evaluating the view on the concrete state
instead gives 15 and 13 distinct states where the reference gives 8 and 4.

**Refusal census (2026-10-27, `main` at #549).** `--impl dive:auto` over
the small tier (156 models, 1 worker), static fallbacks by reason. Default
profile: 144 agree, 7 expected divergences, 5 unsupported; 48 models ran on
the core, 101 fell back, 7 ended before any search. `--compat=tlc`: 150
agree, 6 unsupported; 48 on the core, 100 fell back, 8 ended before a
search. The two profiles refuse for the same reasons; the counts below are
the default profile's, with the compat count where it differs.

| Count | Reason | Owner |
|---:|---|---|
| 55 (52) | temporal properties (liveness checking, the reference engine's own refusal) | lead, M3 |
| 13 | temporal properties (core): mostly implied properties, see §7 | stream 3 (`core/engine-implied`) |
| 11 | SYMMETRY | stream 3 (`core/engine-symmetry`) |
| 4 | VIEW (2) or ALIAS (2) | stream 3 (`core/engine-view`, `core/engine-alias`) |
| 6 | a split argument that is not constant | stream 2 |
| 2 | `--action-composition=tlc` (compat only) | stream 2 |
| 1 | operator arguments | stream 2 |
| 1 | a false or failing ASSUME | stream 2 |
| 1 | `x' \in` an operator building a lazy set | stream 2 |
| 1 | constant operators | stream 2 |
| 1 | a split action inside nested INSTANCE substitutions | stream 2 |
| 1 | INSTANCE substitution by a non-top-level operator | stream 2 |
| 1 | start-up evaluation of `RandomSuccSet` | stream 1 |
| 5 | TLC!PrintT, TLCExt!Trace, Json!ToJson, TLC!TLCGet, TLC!RandomElement | stream 1 |

**Next (M2, stream 3):** widen the core's compat coverage. Shrink the
static refusals that the fallback counts, starting with the most frequent
ones, and replace the comparison guard with stream 2's exact port.

**Next (M1, stream 3):**
1. Scaling measurements on a quiet x86 machine, then profiling of the expand
   phase. Per-worker memo tables are a known cost: each worker warms its own.
2. The small tier under `--engine=auto`, counting fallbacks
   (`08-correctness.md` §8, M1 gate).
