# Changelog

Notable user-facing changes are recorded here. Pre-release development entries
are in git history.

## Unreleased

### Fixed

- `dive simulate` now marks a trace that stopped early at a state with no successor inside the constraints (deadlock checking off, where TLC's simulator also ends the trace) with the new `terminal` ending, in trace document version 8. Sample validation, `validate-trace --sample` and the conformance harness regenerate every successor of the last state and accept the word only if none is admitted and every excluded successor satisfies the invariants and action properties (the same now holds for a sampled `deadlock`); previously such a word was labelled `behavior` and could not be told apart from a truncated one, so the harness graded EWD687a_anim `disagree`. A reader that accepts dive-trace versions 1 to 7 must add version 8 to read these words.

## 0.2.0

### Highlights

- Fairness checking is much faster. ElevatorLivenessLarge now finishes in about 1.5 s at one worker, down from 449 s in 0.1.0 (TLC takes 1,880 s), and the four- through seven-floor Elevator models produce validated counterexamples where 0.1.0 refused or ran out of time.
- Single-worker checks are faster on the speed-comparison corpus. The geomean dive/TLC time ratio is 0.190x, down from 0.195x, and AllocatorImplementation now runs faster than TLC at 0.96x (re-measure on RC).
- EmergencyCall F3 at eight workers now runs as fast as TLC (0.96x, down from 1.60x) with 7% of TLC's peak memory (re-measure on RC).
- `dive simulate` now checks temporal properties on each trace. EWD840 with its `PROPERTIES` takes 2.9 s for 40,000 traces, 3.6x faster than TLC at one worker and 5.4x faster at eight (re-measure on RC). The new `--workers`, `--trace-dir`, `RandomElement` and unbounded-set sampling are covered below.
- Large models use less memory: −69% for temporal graphs, −42% for resident safety frontiers and −41% for the exact-store index. Some models got dramatically faster: CCF MCabs takes 0.11 s instead of 44.6 s, and the failure detector takes 4.7 s instead of 453 s. LuPastry with 16 identifiers, which 0.1.0 refused, checks in 0.48 s.
- ZooKeeper's Zab model with conditional diagnostics now uses every worker: the four-server configuration takes 52 s at 32 workers, down from 613 s on one worker, against 134 s for TLC.

### Performance

All measurements use the installed `dive check` or `dive simulate`. Each record lists its inputs, host and commands. TLC ratios come from the records and are re-measured on the release candidate. The [full table](docs/developer/performance.md#020-improvements) lists every default-path change with its record, including the trade-offs where a change costs a little time to save memory.

| Model | Before | After | Workers | Record |
|---|---|---|---|---|
| ElevatorLivenessLarge (fairness) | 449.30 s in 0.1.0 (TLC 1,879.76 s) | 1.54 s | 1 | [0.1.0 corpus timing run](conformance/reports/perf/2026-11-18-r2-procedure-08a14fba.json), [action images](conformance/reports/perf/2026-11-22-producer-action-images.json) |
| Five-floor Elevator, validated counterexample | 149.59 s | 36.83 s | 1 | [Streett workspace](conformance/reports/perf/2026-11-22-streett-workspace.json) |
| CCF MCabs, exact store | 44.58 s | 0.109 s | 1 | [frame narrowing](conformance/reports/perf/2026-11-21-ccf-core-frame-narrowing.json) |
| Failure detector (detector_chan96) safety, N=3 | 452.96 s, 203 MiB | 4.70 s, 36 MiB | 1 | [guarded message subsets](conformance/reports/perf/2026-11-21-guarded-message-subsets.json) |
| Speed-comparison corpus, geomean dive/TLC | 0.1949x | 0.1896x | 1 | [before](conformance/reports/perf/2026-11-18-r2-procedure-08a14fba.json), [after](conformance/reports/perf/2026-11-20-r2-procedure-c14687e6.json) |
| EmergencyCall F3, dive/TLC time | 1.60x (c7i.metal-24xl) | 0.96x, 403 MiB vs TLC 5.6 GiB (c7i.4xlarge) | 8 | [before](conformance/reports/perf/2026-11-20-emergencycall-reference.json), [after](conformance/reports/perf/2026-11-21-allocator-fixed-large-baselines.json) |
| `dive simulate` EWD840 with `PROPERTIES`, 40,000 traces | 21.40 s (first implementation, unreleased) | 2.92 s (TLC 10.40 s) | 1 | [simulation throughput](conformance/reports/perf/2026-11-21-simulation-throughput-f685076b.json) |
| Flint two-barrier, `--compat=tlc` | 102.16 s, 538 MiB | 83.49 s, 370 MiB | 8 | [label guards](conformance/reports/perf/2026-11-22-core-label-guards.json) |
| Temporal graph memory, c1cs 2M-state prefix | 2.44 GiB | 0.75 GiB | 8 | [edge ordinals](conformance/reports/perf/2026-11-20-temporal-edge-ordinals.json) |
| BPConProof 1M-state prefix, peak memory | 1.12 GiB | 0.46 GiB, and 4.64 s → 2.68 s | 8 | [high-fanout waves](conformance/reports/perf/2026-11-20-high-fanout-waves.json) |
| Trace validation, TCommit with 4,096 managers | 403.35 s | 0.04 s | 1 | [table pair validation](conformance/reports/perf/2026-11-21-table-pair-validation.json) |
| ZooKeeper Zab four-server, `--compat=tlc` | 612.76 s (ran on 1 worker) | 51.74 s (TLC 133.97 s) | 32 | [ZooKeeper reference](conformance/reports/perf/2026-11-20-zookeeper-reference.json) |

### Breaking

- JSON reports from every command now carry `schema_version: 4` (`dive_api::REPORT_SCHEMA_VERSION`). Schema 4 removes four metrics that were always zero: `core.metrics.store.memory.construction_history_view_bytes` and `core.metrics.candidate_storage.{origin_index_bytes, peak_origin_index_bytes, origin_index_bytes_limit_per_worker}`. To migrate, accept version 4 and stop requiring those fields. Trace document versions are unchanged. See [report compatibility](docs/user/reports-and-errors.md#report-compatibility).
- Core checkpoint archives use format `DIVECC02`. A `DIVECC01` archive is refused with `DIVE-S0400`; recreate it with the current executable.
- `dive_trace::TraceDoc` and `Trace` gain `witnesses` and `concrete_closure` fields, and `dive_sim::Trace` and `Report` gain `temporal`. Code that builds these structs field by field must set them.

- Rust API: `SimulateRequest` is `#[non_exhaustive]`. Build it from `Default::default()` and assign fields; struct expressions no longer compile outside dive.
- Distributed workers must run the same dive build as the coordinator.

### Changed defaults

- `dive simulate` keeps and validates the deciding trace plus the first 10 other traces. `--keep-traces all` keeps every trace.
- `dive simulate` no longer stops at 1,000,000 initial or retained states or 10,000,000 generated states. The time and memory budgets now apply instead.
- `dive simulate` samples sets it cannot enumerate, such as `x \in Nat`, using stated seeded distributions. Each drawn member is checked by trace validation. `--sample-unbounded off` and `--compat=tlc` keep TLC's evaluation error 1000. See [unbounded sets](docs/user/simulation.md#unbounded-sets).
- `dive simulate` does not report `<>[]P` as violated on a trace that satisfies it, which TLC's `-simulate` can do. See [D-043](docs/tlc-differences.md#d-043-eventually-always-properties-on-sampled-traces).
- Some temporal checks with `--workers` above 1 now build the behavior graph with one worker. This applies to models with effects or run-time names, such as `SpanTreeRandom.cfg` and `MCKVsnap.cfg`, which were refused before. It also applies to captures with a wide initial frontier and short actions, which run faster this way. `options.workers.reason` and the report header say why.
- The CLI allocator is mimalloc 3.7.0. It fixes slow 16 GiB arena setup on large Linux searches. See [large Linux heaps](docs/user/storage-recovery.md#large-linux-heaps).

### Added

- `dive check --strategy bounded-path --path-bound K` checks safety through K explicit `Next` transitions and replays each witness independently.
- Ordinary Core checks [strong fairness and groups of fairness alternatives](docs/user/temporal.md#fairness) without expanding every alternative, including the four- to seven-floor Elevator counterexamples.
- Temporal state-label refusals name the property, source expression and evaluation error. Core also reports [temporal analysis time](docs/user/temporal.md) by phase.
- `dive simulate --workers N` runs in parallel and reports exactly what one worker reports. `--traces unlimited`, `--keep-traces`, `--max-report-bytes`, `--max-generated` and `--constructive-init` control the run. See [simulation](docs/user/simulation.md#workers-trace-counts-and-what-the-report-keeps).
- `dive simulate` checks temporal `PROPERTY`s on each trace and reports a validated lasso, stuttering or bad-prefix counterexample. `--liveness-extend` and `--liveness-budget` control this check. See [temporal properties](docs/user/simulation.md#temporal-properties).
- `dive simulate` supports `RandomElement`, `RandomSubset` and `RandomSetOfSubsets`, accepts `SYMMETRY`, `VIEW` and `ALIAS`, and labels each step with its action.
- `dive simulate --trace-dir DIR` writes each trace to its own file, in dive or TLC `-dumpTrace` layouts. Traces are validated and written one state at a time, so a two-million-state trace peaks at about 1 GB instead of 8.5 GB.
- Trace documents gain the version-7 `bad-prefix` ending, and `--sample` validation accepts configured temporal properties. See [bad prefixes](docs/user/traces.md#bad-prefixes).
- Independent trace validation scales to large status tables and longer periodic words. It validates the 251-floor Elevator witness and the KIP-996 fair election witness.
- Core can check message-subset actions above the old powerset limit, bounded existential actions near the constant-folding limit, finite record message types, wide integer powersets, and finite predecessor function equations such as the LuPastry+ routing table.
- The bundled [FramedFunctions module](docs/user/modules.md#enumerating-functions-with-fixed-coordinates) generates finite function updates on their writable coordinates.
- Multi-worker Core safety checks run specifications with conditional diagnostics and preserve the printed order. Exact searches can report [orbit sizes and labeled-state counts](docs/user/checking.md#count-labeled-states-under-symmetry) under symmetry.
- Core supports inclusive subsequence predicate searches and `Quantify` in temporal predicates. Scalar CFG assignments can close input-dependent definitions used through a module instance.
- Rust embedders can recheck edited safety invariants and temporal properties on a completed Core check. See the [Rust API guide](docs/user/rust-api.md).
- The user guides build as an mdBook. A new [install](docs/user/install.md) page covers crates.io, Linux release archives and source builds.

### Experimental

These options are opt-in and may change.

- `dive check --lncheck cold-fair` searches for a fair word from the first admitted Init state.
- `dive check --lncheck periodic-all` finds a violation of any configured temporal property on a completed BFS level.
- `dive check --partitioned-fingerprints` supports parallel disk membership beyond four billion entries. See the [storage guide](docs/user/disk-seen-safety.md#multi-worker-fingerprint-membership).
- Core safety searches can store [independently compressed frontier chunks](docs/user/storage-recovery.md#independently-stored-frontier-values) on disk.
- `dive check --periodic-min-states N` sets the first size threshold for [scheduled liveness checks](docs/user/temporal.md).
- `dive validate-trace --repair` searches short paths through an edited source to [repair a saved temporal counterexample](docs/user/temporal.md#repair-a-saved-counterexample-after-an-edit), and validates the replacement independently.
- Projected fairness labels keep earlier source rows when a later source needs ordinary evaluation.

### Fixed

- Under `dive simulate`, [`TLCGet("config")`](docs/user/simulation.md#the-runs-configuration-record) reports `mode |-> "simulate"` (`"generate"` with `--generate`) with TLC's `traces`, `depth`, `aril` and `sched` fields, where it reported `"bfs"`.
- `dive simulate` binds the model as `dive check` does, so a large constant `CHOOSE` such as `EWD998ChanID`'s seven-node ring no longer costs seconds before sampling starts.
- Trace validation replays action composition (`A \cdot B`) whose operands use `EXCEPT`, function constructors or `LET`, so `dive simulate` validates its kept traces of `FiniteMonotonic/MCCRDT` and `MCReplicatedLog`.
- `dive check` applies the Examples Hanoi `Bits.class` override to state-dependent `Bits!And` calls, and `dive simulate` runs models that use it.
- An unsupported Core option such as `--chaos` gets its named refusal even when the process starts above its memory ceiling.
- Independent trace validation certifies `Next` errors, partial successors and deadlocks when replaying a direct diagnostic action reaches no output. See [error endpoints](docs/user/traces.md#error-endpoints).
- If the operating system refuses the worker team at startup, the calling thread completes a multi-worker safety search.
- Core grows the in-memory fingerprint table one shard at a time and bounds parallel candidate batches, so the eight-worker KVS Large check finishes within 24 GiB.
- `dive simulate` releases each trace's states once its report entry is written: bosco at 1,000 traces of depth 100 peaks at 0.36 GB instead of 4.0 GB, with byte-identical reports.
- [D-047](docs/tlc-differences.md#d-047-forced-boolean-formals): an operator argument used as an action is expanded even when an earlier conjunct already evaluated it, so dive can count identical successors that TLC skips.

### Not in this release

Symbolic proofs (`--strategy symbolic`, `finite-induction`, `occupancy`, `compiled`) are not part of the installed 0.2.0 product. They ship in 0.3, working from the release archives, `cargo install` and Sail.

## 0.1.0

The first release.

### Compatibility

- Library users: `dive_api::TemporalMode` no longer has a `FinalShort`
  variant, which Core never implemented, and is `#[non_exhaustive]`. The CLI
  is unchanged: `--lncheck final --lasso short` (and `dive tlc -lncheck final`)
  still refuses temporal properties by name with exit 3, and a model without
  temporal properties is checked exactly as without the flags.
- JSON reports from every command carry `schema_version: 3`. Pre-release
  builds wrote schema 2; schema 3 replaces the single
  `options.temporal_bounds.atom_cells` limit with the separate
  `cell_limits.graph_labels`, `cell_limits.attribution_rows` and
  `cell_limits.word_truth` limits. Readers that pinned schema 2 must move to 3.
- Counterexample documents use `"format": "dive-trace"` with a version that
  depends on how the trace ends: 1 for the ordinary endings, 2 for an error
  while evaluating a named invariant, 3 for a Next step that leaves variables
  unassigned, 4 for an error while checking a generated candidate state, 5 for
  an error in a temporal property's source row and 6 for an error while
  evaluating VIEW. Readers accept all six and reject unknown versions. See
  [trace validation](docs/user/traces.md).
- Library users: the public result, error and kind enums of `dive-api`,
  `dive-runtime`, `dive-trace`, `dive-sema` and `dive-ir` are
  `#[non_exhaustive]`, so a `match` on them needs a wildcard arm. Later
  releases can add variants without a breaking change.

### Added

- Native TLA+ parsing and semantic analysis, supported PlusCal translation,
  and explicit-state model checking with no JVM.
- One checking engine, Core, behind `dive check`, `dive simulate`, `dive dfid`
  and the `dive tlc` shim.
- Safety and temporal checking, including weak and strong fairness, with
  symmetry, VIEW, CONSTRAINT and ACTION_CONSTRAINT support.
- Counterexamples independently validated against their specifications by
  `dive-trace`, which shares no search code with the checker.
- Schema-3 JSON operation reports, per-command help without input files, a
  one-line description of each command in `dive --help`, and `--version`
  naming the release tag or, for an untagged build, the commit.
- Distributed safety checking across worker processes (`dive check
  --distributed-worker`, `dive worker`) over authenticated TCP on Linux and
  macOS, with the same results as a local run and named failures; see the
  [distributed safety guide](docs/user/distributed-safety.md).
- Exact state storage (`--store exact`) for safety and temporal checks. The
  behavior graph keeps complete state identities, so states whose 64-bit
  fingerprints collide stay distinct. The human temporal report names the
  store, and a fingerprint run says that no collision probability was computed.
- Run budgets (`--time-limit`, `--max-memory`) that end a run as `stopped`
  (exit 5) with the phase and limit named. The opt-in `--max-states` and
  `--max-depth` ceilings apply to safety and temporal checks; a temporal
  capture stopped by a ceiling reports no counterexample and no holding
  result. There is no default time limit;
  `--max-memory` defaults to half of physical memory.
- Seeded simulation (`dive simulate`) that reports its seed, so `--seed`
  replays the run, and depth-first iterative deepening (`dive dfid`).
- Embedded standard modules, TLAPS and CommunityModules, with local modules
  and explicit search paths taking precedence; ordinary imports need no extra
  library flags.
- Experimental: `dive debug`, a Debug Adapter Protocol adapter over stdio or
  loopback TCP for Core safety checking in both profiles, with source and
  exception breakpoints, forward steps and bounded scalar inspection; see
  [debugging](docs/user/debugging.md).
- Linux x86_64 and aarch64 archives with checksums, both command entrypoints,
  and the bundled libraries' license and provenance notices.
- Published on crates.io: `cargo install dive-cli --locked` installs the
  `dive` and `tlc` commands, and `dive-api` with its library crates is
  available to Rust callers ([Rust API guide](docs/user/rust-api.md)).

### Results on the tlaplus/Examples corpus

- Of the 176 in-scope models with a TLC result, Core agrees with TLC on 162,
  matches a registered difference on 7 and refuses 7 by name, with no
  disagreements. Seven public real-world specification repositories also show
  no disagreements.
- At one worker, end to end including TLC's JVM start-up, Core takes about a
  fifth of TLC's time on the geometric mean (0.195x over 169 models), and no
  model takes more than twice TLC's time. The
  [measurement procedure](docs/developer/performance.md#r2-measurement-procedure)
  runs one checker at a time on quiet machines.

### Differences from TLC

- The default profile uses canonical TLA+ semantics where TLC's behavior is
  an implementation artifact: unbounded integers, canonical CHOOSE and print
  order, canonical exploration order, and mathematical comparisons of values
  TLC rejects. Each difference is registered in
  [docs/divergences.md](docs/divergences.md) and described in
  [docs/tlc-differences.md](docs/tlc-differences.md).
- `--compat=tlc` selects the inexpensive TLC-compatible switches; exact
  reproduction of TLC implementation artifacts is not promised, and its known
  gaps are listed in docs/tlc-differences.md.

### Limits

- Unimplemented requests refuse by name (verdict `unsupported`, exit 3);
  budget stops are reported as stops, never as success.
- Not in the first release: checkpoint and recovery breadth,
  TLC coverage and logical costs, spec I/O beyond explicit grants, simulation
  with a temporal PROPERTY, VIEW with liveness, distributed temporal checking,
  and LSP.
- EWD998ChanID imports resolve, but checking refuses it by name because it
  combines VIEW with a liveness property.

### Known issues

- With temporal properties, extra workers may not help and can make a check
  slower: on some models a check at 8 workers took up to twice as long as at
  1 worker, and the report does not say why. Results are the same at every
  worker count. Use `--workers 1` for specifications with temporal properties
  until this is fixed.
