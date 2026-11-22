# Changelog

Notable user-facing changes are recorded here. Pre-release development entries
are in git history.

## Unreleased

- Core reuses [pure computed successor components](docs/developer/core/06-memoization.md#24-definitions-a-next-state-action-reads-primed) across candidates. Ordinary Flint checking improved from 3.97 to 3.48 seconds on its published small configuration.

- Temporal [state-label refusals](docs/user/temporal.md#errors-while-labeling-states) now identify the property, source expression and evaluation error and retain the actual graph counts.

- Core evaluates eligible [fairness actions](docs/user/temporal.md#fairness) across original transitions sharing a source. Three-floor Elevator took 2.04 seconds and the four-floor counterexample took 6.91 seconds on the benchmark host.

- Core keeps the component search available on larger fairness graphs. On six-floor Elevator, Core found a validated counterexample in 73.33 s at 541 MiB peak RSS. [Fairness](docs/user/temporal.md#fairness)

- Core reuses values written by a deterministic successor when checking [fairness on original transitions](docs/user/temporal.md#fairness).

- Single-worker Core checks can reuse original successors to answer [fairness enabledness queries](docs/user/temporal.md#fairness) while preserving warnings, action bindings and stuttering semantics.

- Core sizes two [fairness workspaces](docs/user/temporal.md#fairness) for growing graphs and reads reachable product vertices directly from a dense discovery bitmap.

- Core keeps using closed-source [fairness enabledness](docs/user/temporal.md#fairness) above one million expanded states by reading the original actions for one window of source states at a time.

- Core evaluates finite predecessor function equations in the default profile, including the released LuPastry+ routing table.

- Named symbolic proofs can infer and certify finite layouts for sources without a default layout operator.

- Symbolic proofs accept equality between Boolean-valued table cells and Boolean expressions.

- Core reuses [native fairness labels](docs/user/temporal.md#fairness) across related states and successive checks on a growing graph. Together with component search, this cut four-floor Elevator time to a validated counterexample from 19.08 s to 9.62 s.

- Fresh named symbolic checks try the requested invariant directly before searching for a closed predicate.

- Named symbolic proofs can represent products of finite scalar function choices in Init without enumerating the product.

- Parallel Core checks can generate and validate large finite initial products concurrently while admitting their states in source order.

- Compare a declared permutation of interchangeable model values against an independent, unreduced exact check on the original TLA+ files. The source-checkout driver records both counts and a portable input manifest. [Carrier symmetry](docs/user/checking.md#compare-a-declared-carrier-symmetry-on-the-original-files)

- Named symbolic checks can [repair a rejected predicate](docs/user/symbolic.md#repair-a-candidate-on-the-currently-bound-source) in the same invocation.

- The original-source carrier checker can [certify permutations](docs/user/checking.md#certify-interchangeable-model-values) against every reachable state and successor occurrence, or against supported finite relations. Exact-input repeats authenticate the proof and perform a fresh reduced check.

- Named symbolic proofs can [reuse authenticated transition obligations in a new process](docs/user/symbolic.md#reuse-certified-action-obligations-in-another-process).

- Core names the [formula work limit](docs/user/temporal.md#storage-and-limits) when independent fairness assumptions exhaust temporal decomposition.

- Ordinary Core can [check models with independent strong-fairness choices](docs/user/temporal.md#fairness), including the four-floor Elevator counterexample. Component search also speeds up completed checks with many independent fairness choices.

- Whole-value symbolic checks can save a portable safety predicate and recheck an edited invariant list in a separate Dive or Sail process.

- Transfer complete bounded induction certificates across installed Dive and Sail processes with `--strategy finite-induction`. The receiver checks edited invariants and can reuse authenticated Init and action closure.

### Added

- `dive check --periodic-min-states N` sets the first size threshold for [scheduled liveness checks](docs/user/temporal.md). Reports record the actual completed level, counts and validated word.

- [Automatic strategy selection](docs/user/symbolic.md#choose-a-strategy-automatically) chooses exact population, packed, decision-diagram or Core search for an original TLA+ configuration.
- [Match partial etcd Raft event logs](docs/user/traces.md#match-an-etcd-implementation-log) against the upstream trace and protocol specifications, with independently replayable complete states.
- [Match CCF election and rollback histories](docs/user/traces.md#match-a-microsoft-ccf-consistency-history) with complete observation and protocol replay.

- Symbolic transition lowering reuses common state predicates in named proofs and skips untouched table coordinates in functional updates.

- Named symbolic proofs accept finite Cartesian products, tuple-pattern binders, bounded subset and function choices, and quantified state guards. [Supported models and limits](docs/user/symbolic.md#prove-one-configured-state-invariant)

- [Prove one configured state invariant](docs/user/symbolic.md#prove-one-configured-state-invariant) on the original TLA+ source, including models with isolated unbounded counters. Compatible transition obligations can survive an edit.

- Core checks [groups of fairness alternatives](docs/user/temporal.md#fairness) together on finite behavior graphs.

- Core reports [temporal analysis time](docs/user/temporal.md) spent labeling the graph, finding accepting cycles and attributing failed properties.

- Symbolic checking can derive [finite records and sets as whole values](docs/user/symbolic.md#check-finite-records-and-sets-as-whole-values) directly from a safety model. It checks the original EmergencyCall module and independently validates reported violations.

- Linux releases support [`dive check --strategy compiled`](docs/user/symbolic.md#compile-a-finite-safety-check) for eligible finite safety models. With an installed Rust compiler it searches packed exact states at up to eight workers and independently replays violations.
- Compiled finite safety checks use exact dense ranks for eligible symmetric layouts from 33 through 63 bits, including the 45-bit KVS Large model.
- Compiled safety checks reconstruct unambiguous invariant failures with a second native pass and independently validate the concrete trace. Successful checks use no predecessor table.

- `dive simulate` samples specs that use [`RandomElement` in `Init` or `Next`](docs/user/simulation.md#random-choices). Each trace draws from its own seeded stream, the same at any worker count, and the trace records its draws, which validation checks against each set. A step whose draws give no successor is drawn again rather than reported as a deadlock. Draws in invariants or inside `ENABLED` refuse by name.
- `dive simulate` also draws `RandomSubset` and `RandomSetOfSubsets`. Validation checks each recorded draw from the set's size and membership, without enumerating it, so `RandomSubset(k, [D -> R])` over a large function set validates.

- Exact safety searches can report [orbit sizes and labeled-state counts](docs/user/checking.md#count-labeled-states-under-symmetry) when the specification declares symmetry.

- Core [reclaims wide resident frontiers](docs/user/storage-recovery.md) faster when adjacent states share variable values.

- [Independent trace validation](docs/user/checking.md#consistency-predicates-over-named-processes) can check pairwise consistency over large concrete status tables in one scan and validate or reject single-entry updates without rebuilding the table for every possible process.

- [Whole-value symbolic checks](docs/user/symbolic.md) can recheck an edited invariant list against the exact completed reachable set. A violated invariant receives a fresh breadth-first search and independent trace replay.

- [Symbolic rechecks](docs/user/symbolic.md) can reuse shortest-path layers outside the region affected by a deleted action.

- Core can check actions that choose a [subset of messages for one recipient](docs/user/checking.md), including queues that previously exceeded its powerset limit.

- An exact population search can save a [portable safety predicate](docs/user/symbolic.md#transfer-a-population-safety-predicate-across-edits). A receiver proves the current source and reports failed induction obligations explicitly. Direct or retained population proofs accept larger configured participant sets than counted searches.

- Linux releases include [exact labeled occupancy checking](docs/user/symbolic.md#checking-interchangeable-status-tables). An opt-in source-derived BFS counts concrete states and action witnesses for anonymous finite status tables and independently replays shortest violations.

- Core and trace validation can find [wide integer powerset witnesses](docs/user/checking.md) while checking the empty set and singleton candidates.

- [Symbolic checking](docs/user/symbolic.md#admission-and-reports) can load models with large, unused pure definitions without constructing their values during startup.

- Core checks membership in [finite record message types](docs/user/checking.md) without materializing eligible record unions, and independently replays canonical traces with scalar payloads of different types.

- Core can check larger [bounded existential actions](docs/user/checking.md) by sharing pure instructions across bindings, including near the compiler’s constant-folding limit.

- Parallel Core can [keep large split-action plans from exhausting invariant memo storage](docs/user/checking.md#time-and-memory-budgets) within the existing worker memory cap.

- The [EmergencyCall benchmark](docs/developer/large-models.md#enlarge-the-emergencycall-number-sets) checks four larger source configurations and exports independently validated counterexamples.

- [Direct symbolic induction](docs/user/symbolic.md#direct-induction-on-wider-finite-models) admits 2,048-bit finite models and factors independent forbidden pairs. The original transaction-commit safety properties prove at 1,000 managers.

- [Finite symbolic checking](docs/user/symbolic.md#symbolic-layout-for-nested-tables) groups nested labeled tables by their shared coordinates. The KVS source now proves safety at 102 transaction slots within 512 state bits.

- The bundled [FramedFunctions module](docs/user/modules.md#enumerating-functions-with-fixed-coordinates) generates finite function updates on their writable coordinates. Release archives include the TLA+ source for TLC.

- Default Core skips irrelevant [finite function choices](docs/user/modules.md#enumerating-functions-with-fixed-coordinates) when an action fixes every coordinate except one.

- [Symbolic safety checking](docs/user/symbolic.md#admission-and-reports) supports finite symmetry groups acting on record contents, nested indices and refined sets, including independent proposer and acceptor permutations.

- Finite symbolic checking counts duplicate witnesses from inline state disjunctions in partially assigned actions. Initial membership over a lazy set with unproved enumeration multiplicity gives a named refusal. [Symbolic checking](docs/user/symbolic.md#admission-and-reports).

- [Independent trace validation](docs/user/traces.md#large-unused-symmetry-declarations) can replay labeled protocols that contain a large unused permutation declaration. The original 102-transaction KVS counterexample validates in 0.01 s.

- Native fairness checks can rule out destinations that retain a [state variable required to change](docs/user/temporal.md#fairness).

- Symbolic source-edit sessions reuse earlier breadth-first layers when an added action reaches new states or shortens an existing path. [Edit checking](docs/user/symbolic.md).

- Large native temporal checks allocate [action predicate bits](docs/user/temporal.md#storage-and-limits) only for blocks containing true results.

- Core reuses a borrowed function view for lookups and avoids a temporary heap allocation for small `EXCEPT` updates. [Ordinary EWD998 comparison](docs/developer/large-models.md#check-the-ewd998-ring).

- Multi-worker resident fingerprint search reuses confirmed duplicate lookups during ordered commit. [Measurements](conformance/reports/perf/2026-11-21-safety-worker-commit.json).

- `dive-conformance study simulate` compares `dive simulate` with TLC's
  `-simulate` over a corpus, seed by seed: verdict classes, independent
  validation of every deciding trace, and per-property temporal coverage with
  and without `--liveness-extend`. It is not a gate.

- `TLCGet("-Dtlc2.tool.impl.Tool.cdot")` reads the action composition switch
  in `dive simulate` and its trace validation, as it does in `dive check`.

- `dive simulate` does not report `<>[]P` violated on a trace that satisfies
  it, where TLC's `-simulate` does when `P` is false before the repeating part
  of the trace. See [D-043](docs/tlc-differences.md#d-043-eventually-always-properties-on-sampled-traces).

- Temporal graph capture compresses destination IDs in blocks and preserves each generated edge. [Storage measurements](docs/developer/large-models.md#destination-ids-in-temporal-graphs).

- `dive check --lncheck periodic-all` can find a violation of any configured temporal property on a completed BFS level. [Temporal checking](docs/user/temporal.md#ordinary-checking-in-the-dive-profile).

- Temporal graph capture packs repeated action labels and stores exceptions to source-local successor numbering. [Storage measurements](docs/developer/large-models.md#action-labels-in-temporal-graphs).

- Resident exact safety checking grows its hash index in shards. A complete EmergencyCall F3 check used 41% less peak memory with the same result. [Exact-store measurements](docs/developer/large-models.md#resident-exact-state-identities).

- Resident exact safety checking packs full state identities in blocks. The 242-million-state SlateDB control used 35.2 GiB at peak. [Exact-store comparison](docs/developer/large-models.md#resident-exact-state-identities).

- Core checks a [fairness action’s leading state conditions](docs/user/temporal.md#fairness) once per source when they rule out every successor.

- Core temporal checks share original row IDs across transitions whose targets match the first-discovery representation. [Temporal graph storage](docs/developer/large-models.md#action-labels-in-temporal-graphs).

- `dive simulate` checks temporal `PROPERTY`s on each trace's own behaviors
  and reports a validated lasso, stuttering or bad-prefix counterexample. It
  never reports that a property holds: the report counts, per property, the
  traces that checked it and those that could not, and why. Loops close only
  on equal states, so `VIEW` and `SYMMETRY` no longer hide or invent
  violations. `--liveness-extend` (default: the depth; 0 under
  `--compat=tlc`) continues traces cut by the depth, and `--liveness-budget`
  bounds each trace's check. `dive_sim::Trace` and `Report` gain a
  `temporal` field. See [temporal properties](docs/user/simulation.md#temporal-properties).

- Trace documents gain the version-7 `bad-prefix` ending: a finite word that
  no infinite extension can make satisfy the named temporal property. The
  validator checks it in three-valued logic after the usual safety checks.
  Sample validation (`--sample`) no longer refuses configured temporal
  properties; a valid sample claims only its finite obligations. See
  [bad prefixes](docs/user/traces.md#bad-prefixes).

- Core reduces memory per transition for temporal checks with up to 255 action names. [Measurements and capacity limits](docs/developer/large-models.md#action-labels-in-temporal-graphs).

- Core handles repeated first-order helper calls under quantified fairness within the existing temporal planning budget. The original Kafka KIP-996 liveness model now reaches graph capture. [Temporal checking](docs/user/temporal.md).

- Ordinary symbolic edit sessions reuse proved breadth-first layers across action additions and removals, including transitions that shorten a path. [Eligibility and report fields](docs/user/symbolic.md#check-edits-and-prove-properties).
- Core packs ordinary in-memory safety frontiers by variable and stores consecutive discovery IDs once per level. Peak process memory fell about 42% on the measured SlateDB and DiskSynod prefixes. [Inputs, counts and wall times](docs/developer/large-models.md#resident-frontier-rows).

- Core stores large resident fingerprint sets in fewer bytes per slot. [Memory and throughput at 402 million keys](docs/developer/large-models.md#resident-fingerprint-membership).

- Refined symbolic sessions can reuse a completed search across edits to transition guards, repeated actions and self-loops. They report the edited witness count and check for newly introduced deadlocks. [Symbolic checks](docs/user/symbolic.md#learning-which-members-a-wide-set-needs).

- Core reports the measured ordered commit time in `core.metrics.commit_ms` on completed parallel levels. Earlier reports could show zero after subtracting expansion time twice.

- Parallel disk fingerprint membership skips unnecessary page reads using bounded in-memory filters. [Measured time, memory and lookup counts](docs/developer/large-models.md#external-fingerprint-membership).

- `dive check --partitioned-fingerprints` supports parallel safety search with disk membership beyond four billion entries. The [storage guide](docs/user/disk-seen-safety.md#multi-worker-fingerprint-membership) covers capacity, audit results and the measured throughput cost.

- Core safety checks can use multiple workers on specifications with conditional diagnostics. The coordinator preserves the order and number of printed lines. [Checking guide](docs/user/checking.md#workers-and-stores).

- The installed symbolic checker accepts `--recheck MODULE CONFIG` for successive edits. With `--refine-domains`, a completed revision can also check edited invariants on the same reachable set. Action edits can also reuse shortest-path layers, including when an edit adds reachable states. [Usage and report fields](docs/user/symbolic.md#check-edits-and-prove-properties).

- Ordinary Core safety searches can put [independently compressed frontier chunks](docs/user/storage-recovery.md#independently-stored-frontier-values) on disk and reclaim compound values between worker batches.
- `dive simulate --trace-dir DIR` writes every trace to its own file as it
  finishes, validated and in trace order, and the files are the same at any
  worker count. `--trace-format json|tla` writes TLC's `-dumpTrace` layouts,
  with `ALIAS` records, and `--trace-ids A..B` regenerates selected traces of
  a run. Kept traces are now validated and written one state at a time: a
  trace of two million states peaks at about 1 GB, where it took 8.5 GB. See
  [simulation](docs/user/simulation.md#complete-output).
- `dive simulate` samples sets it cannot enumerate, such as `x \in Nat` or
  `\E n \in Nat : x' = x + n`, instead of failing as TLC does. `Nat`, `Int`
  and `Seq(S)` have stated, seeded distributions (`--sample-mean` sets the
  geometric mean), the report names every set sampled, and each drawn member
  is checked by trace validation. `--sample-unbounded off` and `--compat=tlc`
  keep TLC's error. Trace documents gain per-state `witnesses`, and
  `dive_trace::TraceDoc` and `Trace` gain a `witnesses` field. See
  [simulation](docs/user/simulation.md#unbounded-sets).
- `dive simulate --workers N` runs in parallel and reports exactly what one
  worker reports: the lowest-numbered failing trace decides, and `--traces`
  is the total. `--traces unlimited` (with `--time-limit`), `--keep-traces
  K|all`, `--max-report-bytes` and `--max-generated` control the run and the
  report, and `--constructive-init` builds each trace's initial state instead
  of enumerating Init. See [simulation](docs/user/simulation.md#workers-trace-counts-and-what-the-report-keeps).

- Local parallel safety search reuses worker-interned state values at ordered commit. On the complete Flint two-barrier control this reduced wall time from 21.0 s to 17.9–18.4 s. [Comparisons](docs/developer/large-models.md#ordered-commit-on-wide-states).
- Core reduces the number of sources expanded together in high-fanout parallel safety searches. On BPConProof this cut peak RSS by 59% at one million discoveries. [Measurements](docs/developer/large-models.md#high-fanout-parallel-safety).
- `dive simulate` accepts `SYMMETRY` and `VIEW`, which do not change a
  simulation, checks state-level `PROPERTY` parts on initial states (2108),
  and checks action properties together with constraints. Reports render
  `ALIAS` beside each trace's states and label each step with its action. See
  [simulation](docs/user/simulation.md#what-a-sample-covers).
- `dive_trace::TraceDoc` and `Trace` gain `concrete_closure`
  (`"closure": "concrete"`), which lets a periodic word whose loop closes on
  equal states validate under `VIEW` and `SYMMETRY`. Code that builds these
  structs field by field must set it.

- Core uses one worker for temporal captures with a wide initial frontier and simple actions, and reports why in `options.workers`.
- Core temporal checks use compact successor ordinals while retaining exact trace and saved-graph identities. See the [matched memory comparison](docs/developer/large-models.md#successor-ordinals-in-temporal-graphs).
- Core supports inclusive subsequence predicate searches and folds multikey
  bags in canonical key order. Finite bag results accept materialized record keys with different payload shapes. See [bag fold order](docs/tlc-differences.md#d-031-bag-fold-order).

- Scalar CFG assignments can close input-dependent definitions used through a module instance. See [model input selection](docs/user/checking.md#select-a-closed-model-input-in-the-cfg).
- Temporal predicates can count a filtered set with `Quantify`, sum a function
  over a set, and inspect finite next-state domains in nonrecursive binders.
  See [temporal checking](docs/user/temporal.md).

- Core names the temporal formula planning bound and source location when checking reaches it.

- The Linux release archives include `dive check --strategy symbolic` for
  finite safety checks, validated witnesses and independently verified predicates.

- Symbolic safety searches can [learn the members of wide finite sets](docs/user/symbolic.md#learning-which-members-a-wide-set-needs) with `--refine-domains`.

- Symbolic safety checks support [finite state constraints and ground assumptions](docs/user/symbolic.md#admission-and-reports), including independently validated violations on excluded successors.

- Symbolic safety checks accept [multiple finite initial choices](docs/user/symbolic.md#finite-initial-choices), including duplicate and symmetric witnesses.

- Embedders can qualify a completed native temporal check after retaining its
  first property on unchanged aggregate-state programs. See [retaining a claim](docs/user/rust-api.md#retain-the-first-temporal-property).

- The conformance harness can run selected models without TLC goldens with
  `--observe-missing-goldens` and record their ungraded outcomes. This includes
  the four holdouts excluded because the pinned TLC exceeded its deadline.
- Rust embedders can project an exhausted safety proof onto a smaller
  invariant inventory, including constrained models with aggregate state.
  See [the Rust API guide](docs/user/rust-api.md#drop-an-invariant-from-a-completed-safety-check).

- Symbolic BFS can expand a smaller decision diagram for the same next layer. This cut the original KVS Large check from 135 to 35 seconds on one EC2 core. [Matched runs](docs/developer/large-models.md#results-on-keyvaluestore).
- The optional [finite symbolic checker](docs/user/symbolic.md) searches finite safety models using decision diagrams, rechecks invariant edits and verifies inductive predicates against the current source.
- Rust embedders can capture a finite single-worker Core safety check and
  recheck edited invariants on the original state rows. A qualified
  refutation includes its current independently validated witness. See
  [the Rust API guide](docs/user/rust-api.md#recheck-edited-safety-invariants).

- Rust embedders can also recheck edited temporal properties over a completed
  scalar Core graph. The checker replays ordinary scheduled prefix analyses,
  evaluates fairness on the retained labelled transitions and independently
  validates liveness witnesses. See [temporal property reuse](docs/user/rust-api.md#recheck-temporal-properties).

- Trace documents gain the version-7 `bad-prefix` ending: a finite word that
  no infinite extension can make satisfy the named temporal property. The
  validator checks it in three-valued logic after the usual safety checks.
  Sample validation (`--sample`) no longer refuses configured temporal
  properties; a valid sample claims only its finite obligations. See
  [bad prefixes](docs/user/traces.md#bad-prefixes).

### Changed

- Core reuses unrestricted prefix paths and small product rows when checking alternative fairness conditions. Elevator Large completed in 113.90 s versus 403.59 s with the same counts. [Temporal checking](docs/user/temporal.md).

- `dive simulate` keeps and validates the deciding trace and the first 10
  other traces by default, instead of every trace; `--keep-traces all` keeps
  them all. It no longer stops at 1,000,000 initial or retained states or
  10,000,000 generated states: the time and memory budgets govern. A set it
  cannot enumerate is TLC's evaluation error 1000 instead of a refusal.

### Removed

- JSON reports from every command now carry `schema_version: 4`
  (`dive_api::REPORT_SCHEMA_VERSION`). Schema 4 removes the always-zero
  construction-history metrics `core.metrics.store.memory.construction_history_view_bytes`
  and `core.metrics.candidate_storage.{origin_index_bytes, peak_origin_index_bytes, origin_index_bytes_limit_per_worker}`.
  To migrate, accept version 4 and stop requiring those four fields; readers
  of stored reports may keep accepting 3. Trace document versions are
  unchanged. See
  [report compatibility](docs/user/reports-and-errors.md#report-compatibility).
- Core checkpoint archives move to format `DIVECC02`. A `DIVECC01` archive is
  refused with `DIVE-S0400` by name; recreate it with the current executable.
  Distributed workers must run the same dive build as the coordinator.

### Fixed

- Under `dive simulate`, [`TLCGet("config")`](docs/user/simulation.md#the-runs-configuration-record) reports `mode |-> "simulate"` (`"generate"` with `--generate`) with TLC's `traces`, `depth`, `aril` and `sched` fields, where it reported `"bfs"`. When a spec reads the record, its trace documents carry these fields, so `dive validate-trace` checks `ASSUME`s under the same record.

- `dive simulate` binds the model as `dive check` does in each profile: in the default profile Core evaluates the pure constant definitions its actions use only as values, so a large `CHOOSE` such as `EWD998ChanID`'s ring of seven nodes no longer costs seconds before sampling starts.

- Trace validation replays action composition (`A \cdot B`) whose operands update functions with `EXCEPT`, build functions or use `LET`, so `dive simulate` validates its kept traces of `FiniteMonotonic/MCCRDT` and `MCReplicatedLog` instead of refusing them.

- Updated the CLI allocator to mimalloc 3.7.0. It fixes 16 GiB arena initialization, which could cause large Linux searches to spend most of their time creating small memory mappings. [Storage and measurements](docs/user/storage-recovery.md#large-linux-heaps).

- Ordinary multi-worker Core safety checks reuse their expansion threads across frontier waves. If the operating system refuses the team at startup, the calling thread completes the search. [Workers and stores](docs/user/checking.md#workers-and-stores).

- `dive check` applies the Examples Hanoi `Bits.class` override to calls of
  `Bits!And` whose arguments depend on the state. It used to evaluate the
  module's TLA+ body for them. `dive simulate` now runs models that use the
  override instead of refusing `Bits!And`.

- An unsupported Core option such as `--chaos` now receives its named refusal even if the process begins above its memory ceiling.

- Independent trace validation can certify Next errors, partial successors and deadlocks when replay of a direct diagnostic action reaches no output. [Independent trace validation](docs/user/traces.md#error-endpoints).

- Core grows the in-memory fingerprint table one shard at a time, packs trace
  predecessors, and bounds parallel safety candidate batches. On KVS Large
  the eight-worker checker finished within its 24 GiB memory limit.

- `dive simulate` no longer holds every sampled trace as a JSON tree before
  printing its report. Each trace is kept as its serialized entry and its
  concrete states are released once that entry is written, so memory per trace
  falls about tenfold: bosco at 1,000 traces x depth 100 peaks at 0.36 GB
  instead of 4.0 GB. Reports are byte-for-byte unchanged.
- Library users: `SimulateReport::write_json` writes the report text without
  building the JSON tree that `to_json` returns.
- A temporal check whose model has effects or run-time names (for example
  `SpanTreeRandom.cfg` and `MCKVsnap.cfg`) no longer refuses at `--workers`
  above 1 with "concrete pool capture with effects or runtime names". It
  builds the behavior graph with one worker and gives the one-worker result;
  the report's `options.workers.reason` and the human header say so.

### Performance

- Parallel resident [exact safety checks](docs/user/checking.md#time-and-memory-budgets) skip repeated invariant evaluations for states already committed by an earlier wave.

- Core reuses [process-label guards](docs/user/checking.md#time-and-memory-budgets) across actions. On Flint’s complete two-barrier workload, elapsed time fell by 18% and peak process memory by 31% on eight cores.

- Automatic [Store reclamation](docs/user/storage-recovery.md) charges new allocations above the live Store left by its previous collection. At 250 million Flint states this reduced peak RSS from 11.04 to 8.97 GiB.

- Independent periodic [trace validation](docs/user/traces.md#deeply-nested-predicates-on-a-periodic-word) shares completed checks of pure scoped expressions. The KIP-996 fair election witness now validates in 0.67 seconds under the existing work limit.

- `dive simulate` evaluates temporal properties on each worker's compiled Core machine and reuses atom truths across traces. On `EWD840` with its `PROPERTIES`, one-worker time fell from 21.4 s to 2.9 s (3.6x faster than TLC; 5.4x at eight workers). [Measurements](conformance/reports/perf/2026-11-21-simulation-throughput-f685076b.json).

- Symmetry reduction compares nested function and set values without copying their contents. On the [SlateDB WAL comparison](docs/developer/large-models.md#symmetry-comparison-costs) this reduced eight-worker elapsed time by 10%.

- The finite symbolic checker supports string-valued fields and grouped bounded
  quantifiers in state predicates, including the original TCommit safety model.

- Exact symbolic reachability now supports layouts above 52 bits and returns
  integer state and transition counts throughout its 512-bit admission range.

- Parallel Core temporal checks keep one worker team across graph-capture
  batches. In the CoffeeCan3000Beans eight-worker comparison, elapsed time
  fell from 107.3 s to 49.4 s.

### Documentation

- The user guides build as an mdBook: `mdbook build book`, then
  `python3 scripts/check-book-links.py book/book` checks its links. A new
  [install](docs/user/install.md) page covers crates.io, the Linux release
  archives and source builds.
- The user guides read more plainly: shorter sentences, fewer repeated
  caveats and no maintainer process notes. Stale statements about the removed
  Reference engine, host-input support and final/short temporal checking are
  corrected.
- [Getting started](docs/user/getting-started.md) and
  [temporal checking](docs/user/temporal.md) write the counter example inline,
  so their commands run in an empty directory after a crates.io or archive
  install.
- [Install](docs/user/install.md) describes the `dive --version` output of a
  crates.io build, which names the published commit's first 12 hex digits.
- `skills/dive/SKILL.md` describes how to run dive from an AI coding tool: the
  check command, exit codes, refusals, temporal checking and trace validation.

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
