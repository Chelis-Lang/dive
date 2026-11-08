# Changelog

Notable user-facing changes are recorded here. The first release has not been
published. Earlier pre-release development entries remain in git history.

## Unreleased

- Breaking: `dive-liveness` no longer has its public materialized-product
  study API (issue #791): `product::{Product, Labels, CsrTopology}`,
  `search::{find_lasso, Acceptance, Lasso, Step}`, `formula::Recurrence` and
  `Formula::factor_recurrences`. Core builds the temporal product on the fly and
  never used them. Verdicts, counts and counterexamples are unchanged.
- A JSON report whose run stops before Core starts (a parse error, say) no
  longer labels its `options.checkpoint` with `"backend": "reference"` and a
  1 MiB `max_state_bytes` (issue #791). It names `core` and reports
  `max_state_bytes: null`, as every Core run already did.
- Public Rust items with no caller are removed (issue #791):
  `dive_model::{bind_with_names, bind_with_names_controlled}` (restored name
  tables for reference archives), `dive_explore::explore_checkpoint`,
  `dive_eval::Options::for_profile`, `dive_value::codec::{probe_set,
  probe_fcn, probe_raw_set}`, `dive_value::Set::component_count`,
  `dive_ir::IR_SCHEMA`, `dive_core::generate::is_set_kind`,
  `dive_config::ModelConfig::substituted_by`, `dive_sema`'s
  `Model::module_by_name`, `dive_syntax`'s `GeneralId::is_qualified`,
  `dive_conformance::tlc::result::RunMode` and the unauthenticated
  `dive_core::owned_exchange::Exchange::receive_request` (use
  `receive_request_with_startup`). Behaviour is unchanged.
- A `dive worker` picks up the coordinator's next connection within about
  50 microseconds instead of up to 10 ms (issue #953). Distributed checks
  spend most of their time waiting for these connections, so they run several
  times faster: `MCMajority` over two workers takes about 2.7 s instead of
  18 s. Results are unchanged; distributed checking is still slower than local
  checking.
- `dive-eval` no longer has the coverage collectors the reference engine
  fed (issue #791): `SourceCoverage` and its snapshot/row/pause types,
  `TlcCoverageSnapshot`, `TlcCoverageRow`, `TlcActionCoverage`,
  `Evaluator::new_source_covered`, `pause_source_coverage` and
  `coverage_discovered`. Core never built them. `--coverage 0`,
  `--source-coverage` and the `CheckRequest` coverage fields still refuse by
  name; verdicts and counts are unchanged.
- `dive-store` no longer has the reference checker's in-memory stores
  (issue #791): `StateSet`, `FpSet`, `ExactSet`, `Candidates`/`Resolved`,
  `ResolveOptions`, `shard_bits_for`, `key_hash`, the `frontier` module,
  `trace::TraceLog`, `state::{encode_state, StateDecoder}`, the public
  `collision` module and the `StoreError` variants only they raised. Core
  never used them; `StateFingerprinter`, `state_key`, `StoreMode`, `seen`,
  `spill` and `predecessor` are unchanged, and so are verdicts, counts and
  checkpoint archives.
- Distributed safety workers (`dive check --distributed-worker`, `dive worker`)
  now report failures by name (issue #953). Fewer than two or repeated
  endpoints, and out-of-range `--worker-timeout-ms` or `--distributed-retries`,
  are usage errors (exit 2) before the model is loaded. A wrong key names the
  key as a likely cause; a socket timeout names `--worker-timeout-ms`; a unit
  longer than the worker's own deadline is reported by the worker; a missing
  key file is named; a worker whose own start-up fails (a false `ASSUME`, say)
  names the verdict and its message; and refusals say which option or model feature
  (temporal properties, `SYMMETRY`, `VIEW`, runtime effects, missing `--seed`
  or `--fp`, `--sequential`, ...) is not supported. Results are unchanged.
  The `check` and `worker` help describe each option, and the
  [distributed safety guide](docs/user/distributed-safety.md) covers setup,
  supported checks and failure behaviour.
- The binder's start-up evaluation now also evaluates a repeated pure call
  once when the operator comes from an instantiated module or calls other
  operators, in the default profile. The instance's substitutions count as
  inputs of the call (issue #939).
  `MisraReachability/MCReachabilityTestRandomGraphs` takes about 0.1 s
  instead of 1.3 s. `--compat=tlc` still evaluates every call.
- Core's native liveness atoms no longer re-prove the rows they were just
  admitted on for the `<<A>>_vars` subscript test and set membership
  (issue #940). Results are unchanged; `allocator/AllocatorImplementation`
  runs fewer instructions.
- Core enumerates a constant quantifier domain, such as bosco's
  `\E r \in SUBSET (P \X M)`, once per worker instead of once per state, and
  its cached guards on the bound variable now apply (issue #937). Verdicts
  and counts are unchanged; such checks run faster.
- Core applies a lazily kept function constructor of several bound variables,
  such as `CanServiceCall[e, c]` in `MultiCarElevator`, to its argument without
  building the whole function, as it already did with one bound variable
  (issue #938). Such applications over infinite domains, or past entries whose
  bodies fail, now check instead of being refused; verdicts and counts are
  otherwise unchanged and the checks run faster.
- `dive --help` and the `check`, `simulate`, `dfid` and `tlc` help no longer
  list options that Core always refuses (issue #941): `--lncheck final
  --lasso short`, `--action-composition tlc`, `--allow-io`,
  `--source-coverage`, `--coverage 0`, the `simulate` and `dfid`
  `--workers N` option, and the adapter's `-coverage 0` and `-lncheck final`. They still parse and refuse by name with exit 3, as
  before. The `check` help no longer says that Reference is the default
  engine. See the [command-line surface](docs/user/README.md#command-line-surface).
- The conformance harness pins a real-world holdout corpus (issue #854):
  seven public repositories outside tlaplus/Examples (CCF, etcd raft, TiDB,
  BlockingQueue, JGroups PlusCal, BookKeeper, Kafka), each by commit, with a
  dive-owned manifest under `conformance/holdout/` and the new `manifest` key
  in `corpora.toml` (ADR 0005, amendment 2). Sources stay upstream.
- The `dive-check` crate, the reference checker, is deleted (issue #791).
  Core is the only checking engine; check with `dive_api::check_file` and the
  run contract in `dive-runtime`. Its shared test fixtures now live in
  `crates/dive-api/tests/fixtures/checker/`. Verdicts and counts are
  unchanged.
- The coverage plumbing that only the reference engine fed is removed
  (issue #791): `CheckReport::source_coverage()` and
  `CheckReport::tlc_coverage()`, `CheckOptions::source_coverage`, and
  `dive_model::bind_source_covered` and `bind_controlled_source_covered`. Core
  already refuses `--coverage 0` and `--source-coverage` by name, so no report
  carried coverage; the requests and their refusals are unchanged. The safety
  checking guide no longer describes coverage as available.
- `dive-runtime` and `dive-liveness` no longer export run-contract items
  that only the deleted reference checker used (issue #791): the
  `capture::CheckpointObserver` reached-checkpoint hooks and their
  `CheckpointPhase`, `ReachedCheckpoint`, `CheckpointDecision` and
  `CheckpointExploration` types; `ControlledExploration::from_successful_search`
  and `set_elapsed`; `Collector::with_concrete_pool` and
  `Collector::concrete_pool`; `admission::uses_random` (use
  `uses_random_after_startup`); `budget::memory_readable`;
  `context_verdict`; `class::general_text`; `SeenUsage::creation_failure`;
  the `test-hooks` feature; and `dive_liveness::periodic_backend`, TLC's
  growth trigger, which Core never used (it checks on
  `analyze::periodic::Schedule`). Verdicts and counts are unchanged.
- `dive-liveness` no longer has the reference engine's temporal analyses
  (`analyze::analyze_final*`, `analyze::analyze_observed`,
  `analyze::periodic::analyze_periodic*` and the `OwnedTemporal`
  `final_analysis` and `periodic_analysis` methods; issue #791), and no
  longer depends on the `dive-check` crate. Check a behavior graph with
  `analyze::periodic::external::GraphCheck`, as Core does.
- `dive_api::checkpoint` no longer exports the reference engine's
  `checkpoint::check` and `checkpoint::recovery_seed_fp` (issue #791). Check
  or recover with `dive_api::check_file_checkpoint`. The archive request,
  receipt and format items are unchanged, and `dive-api` no longer depends on
  the `dive-check` crate.
- A temporal check whose configured properties all bind to safety roles (for
  example `PROPERTY [](x >= 0)`) no longer runs a reference-engine safety
  search, whose result was discarded, before Core checks the model (issue
  #791). Verdicts and counts are unchanged; such runs take less time.
- Independent trace validation (`dive-trace`) now refuses by name a word
  whose checks evaluate a primed `TLCGet("level")`, which TLC evaluates at the
  successor's level (issue #895). Replay used to read the source's level
  there and could accept a wrong action-property counterexample. Core's
  checker and simulator already refuse such specs.
- The `dive-sim` crate no longer has the reference simulator
  (`dive_sim::simulate`, `simulate_with_poll`, `simulate_with_backend`,
  `Runtime::simulate` and `Runtime::simulate_with_poll`; issue #791).
  Simulation runs on Core through `dive_api::simulate_file`; low-level callers
  run a backend with `Runtime::simulate_with_backend`. When `dive simulate`
  stops at the retained-state bound after a successor violates an invariant or
  action property, the report now keeps that violation as
  `pending_violation`, as it already did for an initial state.
- `dive simulate` now reports a deadlock when every successor of a state is
  excluded by a `CONSTRAINT` or `ACTION_CONSTRAINT`, as TLC's `-simulate`
  does, instead of `internal-error` (issue #896). Independent validation
  regenerates the last state's successors and accepts the deadlock only if
  none satisfies every constraint. This includes models with
  `CONSTRAINT TLCGet("level") < N`. `dive check` is unchanged: there a state
  with constrained-out successors is not a deadlock. Rust callers can run
  the check with `dive_trace::Session::read_sampled_deadlock`.
- Core no longer keeps the retired construction-history table in its Store
  (issue #791). Checkpoint archives (`DIVECC01`), their Store images
  (`DIVEST01`) and worker exchanges (`DIVEOW01`) keep their byte layout with
  that table always empty; an archive that records construction history is
  rejected with `DIVE-S0400` ("legacy construction history") instead of being
  restored. The report field
  `core.metrics.store.memory.construction_history_view_bytes` is deprecated
  and always 0; schema 3 keeps it, and a later schema version will remove it.
  Verdicts and counts are unchanged.
- `dive_api::temporal::Outcome` no longer has the reference engine's
  `Holds`, `Violation` and `PrefixViolation` variants, and the
  `ValidatedCounterexample` and `ValidatedPrefix` types are removed (issue
  #791). Core reports `NativeHolds`, `NativeViolation`,
  `NativePrefixViolation` or `Safety`, as before.
- The Rust API no longer has the reference engine's in-memory final temporal
  route (`dive_api::temporal::check_final`, `check_final_with_evidence`,
  `check_final_with_policy` and `check_final_with_policies`; issue #791).
  Check a file on Core with `TemporalMode::FinalShort` and read
  `CheckReport::temporal_result`. The new `CheckReport::temporal_failure`
  returns the typed failure of a temporal check that stopped without a
  verdict. Caller-set analysis limits and graph or acceptance policies are no
  longer settable; Core uses its file configuration.
- The Rust API no longer has `Engine::Reference` (issue #791). `Engine` has
  the single variant `Engine::Core`, `Engine::parse("reference")` returns
  `None`, and the `*_engine` entry points accept only Core. Requests that
  only the reference engine ran (source or TLC logical coverage, external
  inputs) stay named refusals. Recovering a checkpoint archive the reference
  engine wrote is rejected with DIVE-S0400, as before.
- In the default profile, `IsFiniteSet` of a record set with an empty field
  and a field that is not a proved set (for example
  `IsFiniteSet([a : {}, b : 0])`) now refuses with "Cardinality/IsFiniteSet
  of a Product with an unproved factor shape or size" (exit 3). Core
  answered `TRUE` when the empty field came first in canonical field order,
  where TLC, which checks fields in its own order, can report an error.
  Products and function sets still stop at the first empty factor in source
  order, as TLC does, and `--compat=tlc` is unchanged.
- Core now checks TLC's start-up normalization of the temporal properties
  before it evaluates the ASSUMEs, as TLC does. A spec whose temporal
  properties normalize to no solutions (TLC's tautology error 2253, exit 77)
  and that also has a false ASSUME was reported as an assumption failure
  (2104, exit 10); it now gets the same named refusal (exit 3) as the other
  start-up tautologies. A false ASSUME without that error still reports 2104.
- A Core safety run with several workers that ends in `Init` (an initial
  invariant violation, error or spill failure, for example) now reports one
  worker used, with the reason, instead of the requested width. `Init` runs
  on one worker and no parallel level started. Verdicts and counts are
  unchanged.
- Core liveness checking no longer stores the product of the state graph and
  a property's tableau; each search recomputes product edges from the state
  graph (issue #909). Results and counterexamples are unchanged. Product
  memory now grows with product vertices only: `bcastFolklore` no longer
  exceeds 24 GB on `ReliableChan`, whose product has 2.6 billion edges.
- Core keys and skips an invariant or constraint conjunct that calls a
  recursive operator or function by the variables the called body actually
  reads, instead of assuming it reads every variable (issue #908). Bodies
  with an effect or a nested `ENABLED` keep the old behaviour. Examples
  `KeyValueStore/MCKVsnap` takes about 9 s instead of 22 s at one worker.
- `dive check`, `dive simulate`, `dive dfid` and the `dive tlc` adapter run
  only on Core (issue #791). `--engine reference` and `DIVE_ENGINE=reference`
  are usage errors (exit 2) that say the reference engine was removed;
  `--engine core` is still accepted and is no longer listed in the help. What
  only the reference engine implemented is now a named refusal (exit 3) with
  no alternative route: bounded final checking (`--lncheck final --lasso
  short`, `dive tlc -lncheck final`, whose refusal now names the mode), source
  and TLC logical coverage, granted external inputs (`--allow-io`) and
  recovery of a reference-engine checkpoint archive (`DIVE-S0400`). Refusal
  messages no longer point to `--engine reference`; Core's temporal failure
  report refusals now read "its report is not implemented on Core".
- The binder's start-up evaluation of a constant definition evaluates a
  repeated call of a pure operator on the same constant arguments once, in
  the default profile, and looks configured operator overrides up directly
  (issue #846). `MisraReachability/MCReachabilityTestRandomGraphs` takes
  about 1.1 s instead of 1.8 s. `--compat=tlc` evaluates every call.
- Core liveness checking no longer converts every generated state to reference
  values when all of a property's atoms evaluate natively; it converts them,
  in the same order, only when a row is first read. A state that has no
  reference value is now refused only if the check needs it.
  `core.metrics.liveness.values` gains `admission`, `deferred_rows` and
  `caught_up_rows`.
- Core now reclaims expanded Store values by default on pure fingerprint
  safety BFS once the measured Store exceeds 256 MiB at a completed level
  (`--store-compact-bytes` still sets the trigger explicitly). Results are
  unchanged; searches whose Store stays below the trigger do no extra work.
  Under the default, a search that reclamation cannot serve keeps its Store
  instead of refusing. JSON `core.metrics.store_reclamation` adds
  `trigger_source` and `skipped`. Reclaimed Stores also keep the cached
  materialization proof of compiled constants, so checking after a
  collection no longer slows down: `NanoBlockchain/MCNanoMedium` with a
  64 MiB trigger previously took twice as long as without it.
- Core is now the default engine of `dive check`, `dive simulate`,
  `dive dfid` and the `dive tlc` adapter, and of `dive_api::check_file`,
  `check_file_tlc_inputs`, `check_file_checkpoint`, `simulate_file` and
  `dfid_file` (`Engine::default()` is `Engine::Core`). What Core does not
  implement is a named refusal (exit 3), never a fallback. `--engine reference`
  (or `DIVE_ENGINE=reference`, or `Engine::Reference`) runs the reference engine
  explicitly until it is retired. Migration: callers that relied on the
  reference default pass `--engine reference` or call `check_file_engine` with
  `Engine::Reference`.
- `--engine auto` and `Engine::Auto` are removed: `--engine auto` is a usage
  error (exit 2) that names the replacement. What Auto ran on the reference
  engine is now a named Core refusal that points to `--engine reference`:
  source and TLC logical coverage, external inputs (`--allow-io`) and
  recovery of a Reference checkpoint archive (`DIVE-S0400`). Check reports no
  longer carry the Auto-only `engine_requested`/`engine_fallback` fields;
  simulation and DFID reports keep `engine_fallback`, now always `null`.
- Bounded final temporal checking (`--lncheck final --lasso short`), multi-worker
  liveness with run-time names and finite modes that reach unreplayed random or
  register effects are named Core refusals; see
  [compatibility gaps](docs/tlc-differences.md#compat-gaps).
- Core checks the subscript of `WF_vars(A)`, `SF_vars(A)` and `<<A>>_vars`
  faster in the default profile when `vars` is a tuple, possibly nested, of
  state variables: it compares the variables of the two states instead of
  building both tuples. Results are unchanged; `--compat=tlc` keeps the
  tuple comparison. Native liveness atoms also find their compiled plan
  without a scan of every atom. On the same host
  `SDP_Verification/SDP_Attack_New_Solution_Spec/MC` takes about 0.4 s
  instead of 0.6 s (TLC: 1.1 s) and `allocator/AllocatorImplementation`
  about 6 s instead of 10 s (TLC: 6 s), with unchanged results.
- Core keeps its start-up evaluation of pure constant definitions when the
  model has temporal properties: the binder evaluates only the definitions
  the temporal properties and the specification's fairness reach, instead of
  binding the model again and evaluating every one. `LoopInvariance/MCBinarySearch`
  (`PROPERTY Termination`) takes about 2.5 s on Core instead of 5.6 s, the
  same as without the property.
- Core computes `SYMMETRY` canonical images faster again: a function's
  image is gathered through a remembered image of its domain instead of
  re-sorted, a function variable is compared with the best image before it
  is built, and recurring state-variable values reuse their remembered
  images. `CheckpointCoordination/MCCheckpointCoordination` drops from about
  124 s to 50 s and `MCCheckpointCoordinationFailure` from 7.0 s to 2.9 s
  (TLC: 158 s and 11.2 s), with unchanged counts. Images of records and tuple
  literals keep their literal form.
- Core simulation evaluates `TLCGet("level")` as TLC's simulator does: 0 in
  `Init`, a state's 1-based trace position in its invariants and constraints,
  and the source state's position in `Next`, action constraints and action
  properties. `ewd840/EWD840_anim` now agrees with its golden. Register reads
  and writes (`TLCGet(1)`, `TLCSet`) still refuse, as does `TLCGet("level")`
  in DFID.
- Core refuses a primed `TLCGet("level")` by name in every mode. TLC
  evaluates it at the successor's level; Core read the source's, which gave
  a wrong action-property violation.
- Core checks the run's time and memory budgets once per 32 units of
  evaluation work instead of at every unit, so `--time-limit` and
  `--max-memory` stops may come up to 32 work units later. Verdicts and
  counts are unchanged.
- Core evaluates a closed constant definition too large to fold once, when it
  compiles the plan, instead of in every state that uses it.
  `LeastCircularSubstring/MCLeastCircularSubstringMedium` drops from its time
  budget to about 1.2 s (TLC: 108 s), with unchanged counts. A definition
  whose evaluation fails is still evaluated, and fails, only where it is used.
- Core computes `SYMMETRY` canonical images faster: images of values inside
  a state that recur across states are remembered per worker, within 32 MiB
  per run, and a set variable's image is derived from its predecessor's.
  `SimplifiedFastPaxos/Paxos` drops from 3.7 s to about 1 s and
  `SimplifiedFastPaxos/FastPaxos` from its time budget to about 18 s (TLC:
  1.1 s and 22.7 s), with unchanged counts.
- `dive simulate` names a temporal `PROPERTY` it refuses as
  `simulation temporal properties: <names>` on both engines, instead of the
  exhaustive checker's `temporal properties (liveness checking)` reason, so
  `ewd998/SmokeEWD998` reports the same deferred refusal as
  `ewd687a/EWD687a_anim`.
- Core applies a built-in value operator passed as an operator argument, such
  as KeyValueStore's `ReduceSet(\intersect, S, b)`: the set operators, `=`,
  `/=`, `\in`, `\notin`, `\subseteq`, the Boolean connectives (still
  short-circuiting), `SUBSET`, `UNION` and `DOMAIN`, as if written in place.
  Applied as an action, or passed to a Java-overridden fold (where TLC itself
  fails), a built-in is still refused.
- Fix: Core reported an internal stale-handle error when a function's domain
  held a value retaining a nested record view, such as KeyValueStore's
  `PermSeqs` over logged operations while a temporal property is checked.
  `KeyValueStore/MCKVsnap` now completes with the pinned TLC's unreduced
  counts (D-005).
- Core evaluates each fairness condition of the specification once per
  liveness analysis instead of once per configured property, reusing the
  completed truth of every `WF`/`SF` condition rather than only the first `WF`.
  `SDP_Verification/SDP_Attack_New_Solution_Spec/MC` (six properties, fifteen
  `WF` conditions) evaluates 206,091 native atoms instead of 1,087,181.
  Verdicts, counts and counterexamples are unchanged.
- Core compares strings and model values by a rank in canonical order that
  the string table maintains, instead of by their text, cutting about 6% of
  the instructions of `CheckpointCoordination/MCCheckpointCoordination`.
  The order is unchanged in both profiles.
- Core refuses the `Json` and `IOUtils` operators (`JsonSerialize`,
  `ToJsonObject`, `IOExec` and the rest) only when a run evaluates them, as
  it does `TLCExt!Trace`: a trace export in a disjunct reached only after a
  violation no longer refuses the model, so `ewd840/EWD840_json` and
  `ewd998/EWD998ChanID_export` check on Core. Core still performs no I/O.
  A run that fails or warns no longer refuses for `Print` when the search
  printed nothing.
- Core evaluates a pure `ASSUME`, and the constant definitions only it reads,
  when it compiles its plan, so the binder no longer evaluates them
  (issue #846); a false or failing `ASSUME` is still reported exactly as
  before. Building a canonical set of tuples is also cheaper for every
  engine. `MisraReachability/MCReachabilityTestAllGraphs` takes about 0.7 s
  on Core instead of 7 s.
- Core frees an invariant's temporary values after each initial state even
  when the invariant builds a definition of its own, such as `Corpus` in
  `LeastCircularSubstring`. `MCLeastCircularSubstringMedium` now peaks at
  about 80 MB instead of 12 GB, with unchanged counts.
- Core evaluates `ENABLED` inside an `INSTANCE` whose variables map to the
  same-named root variables when the query reads a recursive operator or
  function, provided the whole compiled plan has no effects (`Print`,
  `RandomElement`, `TLCSet`) and no call body reaches a nested `ENABLED` or
  composition. Core liveness also evaluates `~ENABLED A` atoms and the
  `allocator/AllocatorImplementation` fairness atom natively, so
  `SpanningTree/SpanTree*` and `allocator/AllocatorImplementation` no longer
  evaluate any temporal atom with the reference evaluator. The report's
  `core.metrics.liveness.atoms` gains `negated_enabled_evaluations` and
  `reference_evaluations` (state and action label demands answered by the
  reference evaluator), and drops the `negated_enabled` rejection count.
- Core no longer evaluates every state-dependent `LET` definition by name
  once a spec uses action composition (`A \cdot B`). A composition now
  re-forces only its own frame's state-dependent definitions when a stage
  moves to another row, so `LET m == Min(...)` in `MCCRDT`'s `Reduction` is
  evaluated once per intermediate state instead of once per function cell.
  `FiniteMonotonic/MCCRDT` runs in about half the time, with the same counts.
  Definitions that read primed variables are still evaluated by name in a
  frame that composes, because each stage enumerates many successors.
- Core evaluates a recursive operator or recursive LET function applied under
  a prime, as a refinement mapping `INSTANCE ... WITH v <- Op(e)` does under
  `[][Next]_vars`, in the successor state. `glowingRaccoon/product` now checks
  on Core; `ewd998/EWD998ChanID` reaches its VIEW-with-liveness refusal. A
  prime or `ENABLED` inside such a definition is still refused.
- Fix: Core passed a recursive call that reads the next state (an action-level
  recursive definition, or a state-level one in an initial predicate) to an
  operator as a value shared between disjuncts that assign different next
  states, losing states. Such a call is now evaluated in each disjunct.
- Core evaluates a definition that an action reads primed, such as
  `totalOpOrder'` in `InnerSerial`, once per distinct value of the variables
  it reads instead of once per candidate of an enclosing `x' \in S`.
  `SpecifyingSystems/AdvancedExamples/MCInnerSerial` completes in about 3 s
  with TLC's counts instead of stopping at its time budget. The cache never
  builds a set or function the action would have used without building it,
  and the copies it keeps are bounded by the memo byte cap.
- Core enumerates `{E \in SUBSET T : \A e \in E : Q}` as the subsets of
  `{e \in T : Q}` when `Q` is pure and does not read `E`, so
  `SpanningTree/SpanTreeTest5Nodes` (2^32 candidate subsets) checks on Core
  with TLC's counts instead of refusing.
- Core checks `SYMMETRY` with genuine temporal properties under the default
  `--symmetry-liveness=unreduced` (D-005), as Reference does: the whole run is
  unreduced and the report says so. `=refuse` and `=tlc` still refuse.
- Both engines check temporal properties when the only `RandomElement` is in
  a constant-level definition fixed at start-up, from the reported seed
  (D-002), so `SpanningTree/SpanTreeRandom` no longer refuses; under
  `--compat=tlc` it gives TLC's counts. A violation of such a model still
  refuses, because the independent validator does not replay start-up draws.
  A definition that a configured `Op <- D` or `C <- D` targets is not fixed:
  each use evaluates its body, so its draws still refuse on both routes.
  Nor is a definition whose start-up value keeps a lazy set (`{x \in S : P}`
  or `{e : x \in S}` over an infinite or deferred domain): each membership
  test evaluates it again, so its draws refuse too.
- In the default profile, Core evaluates the pure constant definitions its
  safety roles use as values once, when it compiles its plan, instead of the
  reference binder, and reuses the results of repeated calls while it does.
  `ewd998/EWD998ChanID_shiviz`'s `RingOfNodes` search drops from 16 s to
  under 1 s. A definition Core does not evaluate within its limits (256 MiB
  of transients, its recursion depth) still gets the binder's value. Core's
  `CHOOSE` frees rejected candidates as it searches.
- Core provides `TLCGet("config")` and the start-up counters to the
  `ASSUME`s, and `TLCGet("config")` during the search, as Reference does, so
  `DieHard/MCDieHardest` checks on Core.
- `dive simulate` no longer refuses `ALIAS`: as in `dive check`, the traces
  show the states themselves and the report notes that `ALIAS` is not
  applied. An `ALIAS` naming no definition still refuses.
- Core refuses `TLCExt!Trace` and `TLCExt!CounterExample` only when a run
  evaluates them, not when a specification merely mentions them.
- In the default profile, a constant definition that reads the environment
  without `--allow-io=env` no longer refuses the model when checking never
  uses it and its evaluation only reads (so `ewd998/SmokeEWD998` reaches its
  next refusal).
- Core checks standard operators passed to higher-order operators
  (`FoldFunctionOnSet(+, 0, f, S)`) and `DyadicRationals!Reduce`, so
  `ewd998/EWD998Small` and `Huang/Huang` no longer refuse. An error inside
  such an operator, and `Reduce` operands outside 32 bits, still refuse.
- Core reports a model that fails to bind from its own binding, instead of
  running the whole check again on the reference engine to confirm it. The
  verdict, message, printed output and warnings are unchanged; a binding
  failure under an option only Core implements (candidate spill, store
  reclamation) is now reported rather than refused. Replaying an evaluation
  error in the initial states or a successor set stops at the first state
  past Core's count.

- Fold closed constant sets of up to 65,536 loop iterations at compile time
  (was 4,096), so type sets such as MCNano's `SignedBlock \cup {NoBlock}` are
  built once rather than in every state. Results are unchanged.
- Core liveness reads a state row for a temporal atom only when the reference
  evaluator needs it, not before every native evaluation; the implicit stutter
  reuses its source row. AllocatorImplementation reads 15% fewer rows and
  CoffeeCan1000Beans half as many.
- Core liveness evaluates prepared temporal atoms natively on state rows that
  hold sets, model values or functions with any keys, when every value is
  materialized. SDP and MCFlashWithMutexLive no longer fall back to the
  reference evaluator for their prepared atoms. Results are unchanged.
- Core liveness admits a temporal atom for native evaluation by Core plan
  facts instead of the reference stability check, and no longer caps plan
  size. CHOOSE over sets, Cardinality, set filters, SUBSET, UNION, records and
  multi-clause EXCEPT now run natively; recursive functions, `^`, `ToString`
  and negated ENABLED atoms still use the reference evaluator. At one worker,
  AllocatorImplementation drops from 101 s to 23 s, MCFlashWithMutexLive from
  40 s to 3.7 s and SDP_Attack_New_Solution from 6.5 s to 2.8 s. Results are
  unchanged.
- Core liveness also evaluates temporal atoms that call recursive operators
  or recursive functions, read memoizable definitions or use `^` natively. A
  native atom demand may spend up to 2^20 evaluation steps (was 4,096) before
  it falls back to the reference evaluator, and an atom that runs out 8 times
  in a row stays with the reference evaluator for the rest of the analysis.
  AllocatorImplementation drops from 14.3 s to 9.9 s at one worker. Results
  are unchanged.
- Core liveness no longer converts every generated state to reference values
  when it admits it to the temporal graph. It converts only a state's new
  strings and the values whose conversion could fail, such as sets of records
  or of mixed kinds, and remembers values already admitted. Refusals and the
  order of runtime names are unchanged. MCEWD687a drops from 2.8 s to 2.0 s
  at one worker (TLC 3.4 s) and CoffeeCan1000Beans from 9.0 s to 7.2 s (TLC
  20.0 s). Results are unchanged.

- Independently validate pure bounded TLA-mode action composition, including
  actual closing edges, deadlock and fairness. Nonstuttering compares the
  original and final states; alternate modes and unproved cases still refuse
  (D-022).

- Prepare native Linux x86_64/aarch64 release archives, SHA-256 checksums and
  bundled-library license notices. No version bump, tag or publication is part
  of this preparation.
- Replace historical workstream narratives with the first-value release gates
  and a user-oriented README.

## 0.1.0

This is the curated draft for the first release, **not a published release**.
The [release gates](docs/RELEASE.md) must pass before it is tagged; the
Unreleased entries above are folded into it at the release candidate.

### Added

- Native TLA+ parsing and semantic analysis, supported PlusCal translation,
  and explicit-state model checking with no JVM.
- One checking engine, Core, behind `dive check`, `dive simulate`, `dive dfid`
  and the `dive tlc` shim.
- Safety and temporal checking, including weak and strong fairness, with
  symmetry, VIEW, CONSTRAINT and ACTION_CONSTRAINT support.
- Counterexamples independently validated against their specifications by
  `dive-trace`, which shares no search code with the checker.
- Schema-3 JSON operation reports, per-command help without input files, and
  source revision information in `--version`.
- Embedded standard modules, TLAPS and CommunityModules, with local modules
  and explicit search paths taking precedence; ordinary imports need no extra
  library flags.
- Linux x86_64 and aarch64 archives with checksums, both command entrypoints,
  and the bundled libraries' license and provenance notices.

### Differences from TLC

- The default profile uses canonical TLA+ semantics where TLC's behaviour is
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
- Not in the first release: distributed workers, the debugger, checkpoint and
  recovery breadth, TLC coverage and logical costs, spec I/O beyond explicit
  grants, simulation with a temporal PROPERTY, VIEW with liveness, and LSP.
- EWD998ChanID imports resolve, but checking gives its named VIEW/liveness
  refusal.
