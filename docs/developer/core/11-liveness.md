# 11. Liveness on the core

Status: phase 1 (L1–L4) built, 2026-10-28; pulled forward from M3.
Preparatory L5 topology, private state-value access and graph-independent
producer/result facts are implemented (§7.1–§7.3).
Checker state retention, completion/results and measured capacity remain L5 work
with the prerequisites in §7.

This document describes how `--engine core` checks genuine temporal properties
(`<>P`, `[]<>P`, `P ~> Q`, `WF`/`SF` fairness). The safety parts of properties
(implied initial predicates and implied actions) already run on the core
(`07-engine.md` §7). Phase 1 reuses the reference graph representation and
its limits. The dated census in §10 records its measured coverage; L5 does
not yet establish broader coverage or capacity.

## 1. What the reference engine does

A temporal run in `dive-api` (`crates/dive-api/src/temporal.rs`) has four
stages, each with its own owner:

1. **Capture** (`dive-check`, `crates/dive-check/src/capture.rs`). The
   ordinary safety search runs with a collector attached and returns a
   `BehaviorGraph`, which has:
   - `states`: one `StateVals` per distinct state inside the model, in
     discovery order;
   - `edges[s]`: the generated successors of `s` inside the model, in
     generator order, each as `{to, ordinal, label}`, then one implicit
     stutter `{to: s, ordinal: None, label: None}` once `s` is fully
     expanded;
   - `concrete[s]`: the original successor values of those edges, including
     successors already seen;
   - `roots`: the distinct initial states inside the model.

   The safety checks (invariants, constraints, deadlock and implied
   properties) run as usual during capture. A safety ending becomes
   `Outcome::Safety` with ordinary counts. Capture is bounded by
   `capture::Limits` (1,000,000 states, 8,000,000 edges, 32,000,000
   generated, 65,536 successors per source).

   `Complete` and `Prefix` wrap the graph with a receipt that only the
   producer can construct (`GraphCompletion::finalize` checks the graph's
   shape against the counters).
2. **Analysis** (`dive-liveness`, `analyze.rs`). `prepare` lowers each
   property's negation to a tableau, once. Then `analyze_captured` does the
   rest on the reference evaluator:
   - evaluates state atoms on `graph.states`;
   - evaluates action atoms and fairness `ENABLED` on the edges, using
     `concrete` for edge targets;
   - builds the product and searches for an accepting SCC;
   - returns a lasso (`states`, `back_to`, `edges`), or nothing.
3. **Scheduling.**
   - Default profile: the analysis runs at completed levels chosen by
     `periodic::Schedule` (D-014; at least 2^16 states, doubling). It
     considers only the first property, and a final check runs at
     exhaustion (`analyze/periodic.rs`).
   - `--compat=tlc`: only the final check runs (`-lncheck final`).
4. **Validation and report** (`dive-api`).
   - `finish_final` and `finish_periodic` rebind the model separately.
   - `dive-trace` validates the lasso twice: in memory, and as the
     serialized document.
   - Only then is the result built as a `temporal::Report`, which
     `check.rs` renders as `Phase::Temporal`. A lasso that does not
     validate is a validation failure, never a verdict.

## 2. Principle: the core replaces only the producer

The core takes over stage 1, exploring and capturing the graph. Stages 2–4
stay the reference engine's code, unchanged:
- planning, and evaluating state atoms, action atoms and `ENABLED` with the
  reference evaluator;
- the tableau, the product, the SCC search and the lasso selection;
- the periodic schedule;
- validation by `dive-trace`, and the report.

The core produces the same graph as the reference capture: the same states
in the same order, the same edges in the same order, and the same roots.
Stages 2–4 are deterministic functions of the graph. So:

- verdicts, the violated property and counterexamples equal the reference
  engine's by construction, including which lasso is chosen;
- counts at a stop are the core's own, already equal to the reference's
  (`07-engine.md`), and periodic checks fire at the same levels, since the
  schedule reads only the graph's size;
- validation stays independent of the core: `dive-trace` checks the lasso
  on a freshly bound model with the reference evaluator (ADR 0103);
- `ENABLED`, fairness, and action predicates in properties are evaluated by
  `dive-liveness`. The core also supports `ENABLED` in safety predicates;
  its remaining INSTANCE-substitution refusal is separate from this adapter.

The strongest test (§8) is therefore graph equality: the core's
`BehaviorGraph` must equal the reference capture's, field by field, at every
worker count.

## 3. Graph capture on the core (`dive-explore`)

A `GraphSink` passed to `dive_explore::explore_with` (`graph.rs`) receives
these events, all from the single-threaded commit, in commit order:

| Event | When | Reference rule (`dive-check/src/engine.rs`, `on_state_checks`) |
|---|---|---|
| `emitted(generated, ordinal)` | every generated state, before its checks (`ordinal` is `None` for an initial state) | `capture.allow_emitted` |
| `state(heap, Step)` | a state inside the model that passed its checks, new or seen. `Step` carries the key, the new id and values (persistent handles) when it is new, the parent, the ordinal and the action index | `capture.record`; a root when `parent` is `None` |
| `stutter(source)` | a source is fully expanded with no stop | `capture.stutter` |
| `level_done(level, stats) -> bool` | the commit of a completed level; `true` stops the search there (`Verdict::PrefixStop`) | `Observer::level_done` and `stop_after_level` |

Each event may return `Stopped`; the search then ends with
`Verdict::GraphStop` and the sink keeps the reason. Vertex ids are the
predecessor log's ids, which are the discovery indexes.

The sink (`dive-api`, `core_graph.rs`) keeps the key-to-id map (`u64` key
to vertex id), to reuse a seen state's values. Two states with
equal keys are the same graph vertex. This is exactly the reference's
fingerprint-mode identity and collision qualification. In exact mode the
core's keys are exact (`07-engine.md`).

How the events follow the reference engine:

- **States outside the model** (`CONSTRAINT`, `ACTION_CONSTRAINT`) produce
  no vertex and no edge.
- **Ordinals** are the generator's emission index within the source.
  `selected_targets` in `dive-liveness` checks them against `concrete`.
- **Labels** are `model.next[action].label()`, the same strings as
  `Successor::action.label()`. The core keeps the action index, and
  `dive-api` maps it to the label.
- **The parallel engine** commits in position order (`07-engine.md` §3), so
  the events, and therefore the graph, are identical at every worker count.
  Expansion stays parallel. Only the event delivery is ordered.
- **Refused at first**, each with a named reason:
  - `SYMMETRY` and `VIEW` with temporal properties. The reference keeps a
    representative plus concrete values, and has its own symmetry-liveness
    rules (`symmetry_liveness`).
  - `POSTCONDITION` with temporal properties (the reference refuses it too).
  - Checkpoints, DFID, `--temporal final-short` and host I/O hooks.

`dive-explore` gains no dependency: the sink is a trait over core handles.

## 4. Conversion to `StateVals` (`dive-api`)

`dive-liveness` reads `StateVals` (`Arc<[Option<Value>]>`). The `dive-api`
sink converts each new vertex once, with
`dive_core::print::to_value(.., Conversion::Report)`, the conversion the
core's traces already use.

- **Sharing.** A memo from a persistent compound handle to its `Value`
  shares structure: a sub-value common to many states becomes one `Arc`.
  Without it, conversion would copy every state in full, which costs more
  memory than the reference capture.
- **Concrete rows.** For each edge, the concrete successor is a clone of the
  `StateVals` of `states[to]` when its handles equal the representative's.
  Persistent handles are canonical, so equal handles mean equal values. This
  is the only case in exact mode. A fingerprint collision, where the handles
  differ, is converted separately, as the reference keeps the original
  value.
- **Cost budget.** Conversion work, in nodes visited, is counted in the
  run's metrics as `liveness.convert_ms`, `liveness.converted_nodes` and
  `liveness.memo_entries`. The memo is bounded by
  `capture::Limits::states` × the variable count × a fixed factor. Past that
  it stops caching rather than grow, so the worst case is the copy cost.
  - Target (directional, `09-performance-method.md`): conversion costs at
    most 25% of exploration time on the corpus's temporal models.
  - The work is charged to the run budget (§7). It can end in a structured
    stop, never in an unbounded allocation.

## 5. Analysis entry points (additive APIs)

Existing callers are unchanged. Both crates are reference crates, so each
change needs the full gate.

- **`dive-check`.**
  - `capture::refusal(model, options, parallel) -> Option<String>` returns
    the reference capture's own admission refusals (VIEW, SYMMETRY,
    POSTCONDITION, checkpoints and the rest), with the same messages and
    order. `explore` calls it too, so the two cannot drift.
  - `capture::GraphBuilder` wraps the internal collector, so the limits,
    limit messages and row layout are the reference's. The producer calls
    `allow_emitted(generated, ordinal)` before each generated state's
    checks, `record(key, state, id, parent, ordinal, label)` for each
    accepted state inside the model (in commit order) and `stutter(source)`
    once a source is fully expanded. `graph()` gives the graph so far.
  - `GraphBuilder::complete(safety, options) -> Result<Complete, Verdict>`
    runs the same `GraphCompletion::finalize` shape checks against the
    producer's statistics. `GraphBuilder::prefix(level, safety, options)`
    builds a `Prefix` and requires an unexhausted frontier. The receipt
    certifies the graph's shape against the counters, nothing more, as
    today.
- **`dive-liveness`.** `analyze::periodic::external::GraphCheck` exposes the
  existing private steps. `periodic_unbudgeted` and the capture observer
  now share them, so the two paths run the same code:
  - `GraphCheck::prepare(model, options, limits, policy, labels, schedule) -> Result<GraphCheck, Box<PeriodicFailure>>`
    arms the run budget (`dive_check::budget::arm`), validates the context,
    builds the evaluator control and plans every property, once.
    `schedule` is `Some(Schedule)` for the default profile and `None` for
    `--compat=tlc`, which runs only the final check.
  - `GraphCheck::level(&mut self, level, &BehaviorGraph, Stats) -> bool`
    applies the periodic schedule and `analyze_captured` with only the first
    property, as the observer does. It keeps the decision and returns
    whether the search stops after this level.
  - `GraphCheck::finish_prefix(self, Prefix)` and
    `GraphCheck::finish(self, Complete)` both return
    `Result<(PeriodicAnalysis, Checks), Box<PeriodicFailure>>`. The first
    turns a kept decision into `PeriodicAnalysis::Prefix`. The second runs
    the final check over all properties. Both take the run budget's final
    sample as the reference does.
  - `GraphCheck::stopped(self, CheckResult)` reports a safety ending or
    stop of the producer's search as `AnalysisError::Exploration`, never a
    temporal verdict.
  - `GraphCheck::row_order(&self) -> bool` says whether the reference may
    evaluate atoms in TLC's row order (§11).

`dive-api` then passes these results to the existing `finish_periodic` or
`finish_final`. Validation and the report are byte-for-byte the reference
engine's path.

## 6. Verdicts, counts and the report (`dive-api`)

`core_engine::check` has a temporal route (`core_temporal.rs`), for
ordinary checking (`TemporalMode::Ordinary`): with the completed-level
schedule in the default profile, the final check alone under
`--compat=tlc`, as `check_backend` routes the reference engine. `prepare`
binds, checks the `ASSUME`s and compiles the plan for both routes; on the
temporal route the binder's temporal-property entry is not a refusal, and
the model is bound with the requested symmetry-liveness policy. When the
model has `implied_temporals`, the route:

1. refuses what the reference capture refuses (`capture::refusal`), what
   the reference's ordinary temporal dispatch excludes
   (`final_roles::route_exclusion`), `Print`, `--max-states` and
   `--max-depth`;
2. prepares a `GraphCheck`, so planning comes before the search, as on the
   reference engine;
3. runs the explorer with the sink, on a thread with the evaluator's stack
   (`RECOMMENDED_STACK`), as the reference runs its temporal checks. The
   safety checks behave as on the safety route, and a safety ending is
   reported as the reference's temporal route reports it (a safety result,
   with the bound model's symmetry resolution);
4. on `level_done`, passes the graph so far to `GraphCheck::level`. A
   candidate stops the search after that level;
5. completes the graph with `GraphBuilder::complete` (or builds the prefix
   with `GraphBuilder::prefix`) and calls `GraphCheck::finish` (or
   `finish_prefix`);
6. returns the report built by `temporal::core_report`, which runs the
   reference's `finish_periodic` (validation and report). A report whose
   outcome is a safety result is returned as one.

For now, a temporal failure is refused rather than reported: a planning
failure, a graph limit or conversion failure (`GraphStop`), an analysis
error and a validation failure. `--engine auto` then reports the
reference engine's failure. This keeps the row-order question of §11 out
of L4: every atom error falls back. Reporting the failures on the core is
later work.

`Fallback::of` treats the temporal route's refusals like any other refusal,
so `--engine auto` falls back with the reason named. The JSON report gains
the core's `liveness` metrics under `core.metrics` (`convert_ms`,
`converted_nodes`, `memo_entries`, `graph_states`), which are not compared.
All other fields equal the reference engine's, except the fingerprint
collision estimate, which the core does not report on any route.

## 7. Budgets

These follow the liveness-budgets rule:
- capacity at least TLC's;
- a structured stop with a stable reason;
- no runaway work.

- **Phase 1 (PRs L2–L4): the reference's limits.** The core uses the same
  `capture::Limits` as the reference capture. A model the reference refuses
  for graph size is refused by the core with the same message. This keeps
  results identical. The armed run budget (`--max-memory`, `--time-limit`)
  covers exploration, conversion and analysis together:
  - `dive-explore` already polls it once per chunk;
  - the sink polls it once per converted state;
  - `dive-liveness` polls it in its stages (`crate::budget::exhausted`).

  Exhaustion is `AnalysisError::Stopped`, reported as a stop with the phase
  in which it happened, never a verdict.
- **Phase 2 (PR L5): measured capacity at least TLC's.** The planned graph
  uses compact vertex ids and CSR edges, with action indices instead of
  repeated labels. On-demand conversion to `StateVals` is planned over the
  state-value source boundary (§7.2) with a bounded cache. Its public interface
  remains unset until a controlled native adapter is implemented.
  - **State and topology access.** `labels`, `selected_targets` and lasso
    extraction use a private state-value source with a reference adapter (§7.2).
    Topology and edge metadata still come from `BehaviorGraph`. Product
    `BehaviorView` supports dense, captured and borrowed CSR topology (§7.1).
    Native state retention, controlled conversion and the end-to-end compact
    backend remain to be built. Keep reference entrypoints unchanged.
  - **Retained states.** The store retains values, but the explorer keeps
    only the current/next frontier's state-handle rows. L5 must retain an
    addressable handle row or state root for every graph vertex. Those
    retained roots must remain valid until analysis and lasso extraction end.
  - **Completion and results.** `capture::Complete` and `Prefix`, and the
    liveness `Analysis`/candidate results, currently retain a concrete
    `BehaviorGraph`. Graph-independent sealed producer facts and private result
    assembly are available (§7.3). A checked native producer constructor and
    compact-compatible report routing remain needed so finishing a compact run
    does not materialize the reference graph. Preserve existing producer
    completion/prefix checks and actual counters; compact storage creates no new
    temporal proof authority.
  - **Complete memory accounting.** The earlier estimate of 12 bytes per
    vertex plus 8 per edge was a topology estimate, not a total footprint
    or a capacity proof. Count retained handle rows/roots, key-to-id lookup,
    CSR construction scratch and growth, original ordinals, action metadata,
    conversion cache, the persistent store, labels and product/search memory.
    Preserve duplicate edges, ordinal gaps and implicit stutters exactly.
  - Replace the core's fixed capture counts with budget-backed retention
    only after those paths have bounded allocations and cooperative stops.
    Preserve explicit reference-library limits. Measure end-to-end capacity
    against pinned TLC on the largest temporal corpus model, under recorded
    comparable memory/time policies, including peak RSS and every phase.
    Record completion, all compared observables and independently validated
    counterexamples in `09-performance-method.md` format. Partial work or a
    resource stop earns no completion or capacity credit.
  - Raising a default limit changes results only from "refused" to a
    verdict. It is result-preserving against TLC and needs no divergence
    entry.

L5 keeps the reference evaluator, temporal analysis and independent validator.
It does not implement quotient-edge or closing-state semantics for VIEW or
symmetry, repair temporal error-demand order, or widen the temporal language
and effects boundary. Those remain separate obligations even if capture fits
in memory. Graph/report equality, both semantic profiles, completed-level
scheduling, independent validation and the full gate remain acceptance gates.

### 7.1 Implemented first dependency: borrowed CSR topology

`dive_liveness::product::CsrTopology` validates borrowed offset, 32-bit target
and root slices without constructing a nested adjacency copy.
`Product::from_csr` and `with_csr_recurrences` use the same product builder as
the existing dense input. The internal compact product uses that topology view
too. The input view retains root/edge order and duplicates; product construction
keeps its existing canonical root handling. Empty rows stay dead ends, and
witness edge numbers remain original per-row positions. Existing
product limits and acceptance-buffer policies retain their dense-input results.
This view validates references, not producer completion or temporal acceptance.

This is the topology dependency, not compact checker graph capture. Analysis
reads reference representatives and original targets through the private source
boundary (§7.2); topology, metadata and Complete/Prefix still retain the reference
graph. Next: retain all-vertex roots, implement controlled conversion/cache and
compact-compatible completion/results before routing the checker through CSR. Concrete targets must preserve the original generated values; substituting
`states[edge.to]` is not equivalent. Retained original target values or their
exact reconstruction/cache must enter the memory and work accounting. Its
builder must retain action/ordinal metadata and poll/account allocations
under the run budget. No graph limit is raised and no capacity or throughput
claim follows from this product-input support.

### 7.2 Implemented state-value access prerequisite

The private `analyze::state_source::StateSource` separates representative values
and original concrete successors from graph topology. The production analyzer
uses `CapturedStates`, which borrows the existing `BehaviorGraph`; reference
entrypoints, graph ownership and completion/result receipts stay unchanged.
Representatives may borrow an existing row or own a lazily converted row.
Original metadata can be inspected without converting its values. No public
conversion, cancellation or error interface is established by this extraction.

`selected_targets` checks ordinal/label correspondence in original row order,
keeps the first original successor for each destination, and reads no later
duplicate's values. Implicit stutters use their source representative only when
that destination has not already selected an original. Labels retain per-atom,
per-vertex demand and action evaluation once per destination. Inactive action
atoms and empty frontier rows request no values. Lasso extraction reads only
the chosen representatives and retains original edge positions and metadata,
including through a row-cache projection. Source failures and typed stops
propagate as errors; they never mean absence of an accepting word.

Tests compare the reference adapter with a lazy owner of separate value rows:
complete packed labels, candidate states, edges, property attribution and
lasso positions agree in both profiles, including `ENABLED`, WF and SF. They
also cover differing originals for one destination, ordinal gaps, malformed
metadata, untouched atom-error priority, every demanded read's failure/stop,
and a mutation that incorrectly replaces original values by representatives.

This is not native compact capture. Before a core source can implement the
boundary, capture must expose original targets even for seen states: `Step`
currently carries values only for new vertices, and parallel `Occ::Old` omits
them. Retain persistent handle rows for every vertex and any differing original
target, keep their store alive, and bound both committed and speculative
retention. Controlled conversion/cache, compact-compatible completion/results,
CSR metadata and end-to-end memory/capacity measurements remain §7 requirements.
The private source itself supplies no resource bound or completion authority;
no checker limit, supported model population or performance claim changes.

### 7.3 Implemented producer-fact and result assembly prerequisite

`capture::CompleteEvidence` retains the existing `GraphCompletion`, the actual
ordinary safety result, original options and counters without graph rows.
`PrefixEvidence` retains actual successful committed-prefix facts and its level,
with a nonzero queue and no completion receipt. Their constructors remain
private. `Complete::evidence`/`Prefix::evidence` borrow these immutable facts;
`into_evidence` releases graph storage while retaining them. Edits to public
capture fields do not change the original facts or recertify completion.

The reference producer already distinguishes a successful unexhausted committed
prefix from a safety failure or resource stop. The public `GraphBuilder`
factories now preserve that distinction too: any non-success safety result
returns its exact original verdict, before constructing either kind of evidence
(#624). Successful completion still uses the existing `GraphCompletion::finalize`
shape/counter checks; successful prefixes retain the nonzero-frontier check.
No CSR shape, copied counters or caller completion flag creates these facts.

Private liveness result assembly consumes the existing candidate word or error
and the matching kind of producer evidence. Only complete evidence can assemble
a holding decision. Prefix assembly requires a selected word, and analysis
errors and typed stops remain errors. Existing final-only and periodic paths
use this boundary. Their compatibility adapters keep the original public
`Complete`, `Prefix`, `Analysis`, candidate and API `Outcome` values, retaining
the original graph allocations without copying or rebuilding rows.

Tests detach real producer evidence and observe graph rows being released,
preserve every selected word/edge and legacy graph allocation, reject malformed
completion counters/rows, preserve original factory failures/stops, and keep
successful joined prefixes at 1, 4 and 8 workers in both profiles. Public
rustdoc controls prevent constructing completion facts or promoting prefixes.

This prerequisite does not implement a native compact producer constructor,
compact final report routing, retained handle roots or controlled conversion.
The legacy public results still retain `BehaviorGraph`. The future store/source
bridge must provide checked producer facts and the original selected values;
it must not rebuild a reference graph just to assemble a result. No capture
limit, supported model population or capacity claim changes (#625).

## 8. Tests

- **Graph equality** (the main test). For every temporal fixture (the
  liveness fixtures of `dive-liveness` and `dive-api`, and the
  `configured-temporal-operators` corpus), the core's `BehaviorGraph` equals
  `capture::explore`'s field by field, at 1, 2, 3 and 8 workers, in both
  profiles. The test lives in `dive-api` (`tests/core_liveness.rs`).
- **Report equality.** The JSON report from `--engine core` equals the
  reference engine's, except `engine`, `core` and timings, for every
  fixture above. That covers holds, violations with validated lassos,
  periodic prefixes, and safety endings during capture.
- **Budgets.** Fixtures where `--max-states`, `--max-memory` and capture
  limits trip before, during and after capture each end in the reference
  engine's stop.
- **Harness.** `--impl dive:auto` over the small tier in both profiles:
  - no regression against `conformance/baselines/`;
  - the count of temporal models that move to the core (§10);
  - the shadow comparison (core against reference) on those models.

## 9. PR plan

| PR | Scope | Gate |
|---|---|---|
| L1 (built) | This design; status board and plan entries | fast |
| L2 (built) | `capture::refusal` and `GraphBuilder` (`dive-check`); `GraphCheck` (`dive-liveness`); tests: a graph rebuilt through `GraphBuilder` equals the reference capture, and `GraphCheck` over it equals `analyze_final` and `analyze_periodic` | full |
| L3 (built) | `GraphSink` in `dive-explore`; the `dive-api` converting sink; graph-equality tests (§8) against the reference capture | fast |
| L4 (built) | The temporal route in `core_engine` (both profiles), refusals, report-equality tests, harness census, user guide and CHANGELOG | fast (full if L2 needs follow-up) |
| L5 (partial) | Borrowed CSR product input, private state-value source and graph-independent producer/result facts built (§7.1–§7.3); native retention/conversion, checked compact producer completion and report routing remain, followed by TLC capacity measurement (§7) | full |

Each PR updates this document's status, the status board and the handoff log.

## 10. Census and results

**Before L4.** The core refused every genuine temporal property
statically ("temporal properties (M0 core engine)"), so under `--engine
auto` every such model ran on the reference engine.

**After L4** (2026-10-28, `--impl dive:auto`, small tier of examples,
probes, validation-smoke and configured-temporal-operators, 1 worker, TLC
budgets capped at 900 s; harness reports kept outside the repository):

| Profile | models | agree on the core | agree after a fallback | unsupported | disagree |
|---|---|---|---|---|---|
| default | 156 | 114 (+4 expected divergences) | 28 (+3 expected divergences) | 5 | 0 |
| `--compat=tlc` | 156 | 108 | 40 | 6 | 0 |

- 31 models with genuine temporal properties now run on the core and agree
  with the goldens in both profiles: 29 from the examples (among them
  CoffeeCan1000Beans, 501,500 distinct; TokenRing, 46,656; nbacg_guer01,
  24,922; ReadersWriters, 21,527; and MCRealTimeHourClock, a liveness
  violation) and the two configured-temporal-operators models.
- Indicative timing, CoffeeCan1000Beans at one worker on M3: 0.77 s and
  94 MB on the core, against 2.28 s and 134 MB on the reference engine.
  The conversion took 152 ms for 501,500 states.
- The temporal models still on the reference engine are refused for
  constructs the core does not run yet: `ENABLED` (6), `SYMMETRY` with a
  temporal property (P4SymLive), and other refusals of the safety route.

## 11. Risks and open points

- **TLC row order for atom errors.** On the reference engine, a run at one
  worker evaluates its state atoms during capture, at TLC's checkpoints
  (`analyze/row_demand.rs`), whenever:
  - the run owns its model;
  - labels are uncapped (`GraphLabelPolicy::NoCellLimit`);
  - every atom is a state predicate.

  An atom that fails to evaluate then ends the run where TLC would, with
  TLC's counts. The core evaluates atoms after capture (or at a due level).
  For holding and violated properties the result is the same, because the
  row cache only saves work. An atom error can differ, though.
  `GraphCheck::row_order` reports whether a row program may be active (a
  conservative superset of the reference's condition). If it is,
  and the analysis then fails on an atom, the core returns `unsupported`
  ("a liveness atom error in TLC's row order (core)"), and `--engine auto`
  falls back after the search ("dynamic", as for other dynamic refusals).
  Reproducing the row order on the core is later work, if the corpus shows
  a need.

- **Label strings.** If a core action's label differs from the reference's
  `Successor::action.label()` (for example, split actions inside INSTANCE),
  the graph-equality test fails and the model is refused until the two
  agree. Labels are never derived independently.
- **Conversion memory in phase 1.** The memo bounds the cost, but a model
  near `capture::Limits` holds both the core store and the converted values.
  Phase 2 removes that duplication.
- **Symmetry and VIEW liveness.** A temporal property with `SYMMETRY`
  is refused for now; it will follow the reference's `symmetry_liveness`
  resolution. SYMMETRY under `--compat=tlc` (#574) and VIEW with SYMMETRY
  (#576) run on the safety route.
