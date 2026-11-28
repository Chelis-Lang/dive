# Scoreboard

Current measurements against the [release gates](RELEASE.md). Replace rows when
a measurement is repeated; history is in git.

## Release gates

| Gate | Status | Evidence |
|---|---|---|
| R1 Coverage | **Met on main.** Core alone, default profile, one worker, full Examples tier including the long tier: 169 covered (162 agree, 7 registered expected divergences), 0 disagreements, 7 named refusals and no stops. The frozen historical floor on the same selection rule is 140 (see the [R1 gate](RELEASE.md#gates)). 48 models are out of scope and 9 have no TLC golden. | Quiet-node run in #784; #338 |
| R2 Speed | **Met on the 0.2.0 candidate `5bb88063`** (#1418) under the [R2 measurement procedure](developer/performance.md#r2-measurement-procedure): one checker at a time on quiet nodes, small models as the median of six alternating repetitions. Geomean 0.170x TLC over the 169 agreeing models (fixed-set geomean 0.185x, counting the six excluded models at 2x); no model above 1x. The highest is `EWD998Chan` at 0.87x. | `conformance/reports/perf/2026-11-27-r2-procedure-5bb88063.json`; [release results](developer/release-performance-results.md) |
| R3 Bare use | **Met on Core.** All ten smoke models resolve with no library flags; nine check and EWD998ChanID refuses by name. The CI conformance gate and the x86_64 release archive with an empty `PATH` both pass on main. | #803; #784 |
| R4 Compat exists | **Met.** `cargo xtask gate --full` passes on main. In that run Core in the TLC profile ran the small tier with 142 agreements, 13 named refusals, one registered gap and no internal errors; the committed TLC-profile baseline, refreshed from a later run with the same counts, records them as the ratchet floor. | #810; #784 |
| R5 Stable surface | **Met.** Command help, `--version` naming the source commit, hidden unimplemented commands (`lsp`); the help and schema-4 JSON-report golden tests pass on main. | #802, #804; #784 |
| R6 Shipped | **Met.** v0.1.0 is released: the GitHub Release has both Linux archives with checksums, all 18 crates are on crates.io, and `cargo install dive-cli --locked` works in a clean Cargo home. | #794 |
| R7 Sail with dive | **Met.** Through Sail 0.3.1 with dive 0.2.0, a fresh DieHard check is 36.8x faster than TLC (12.3 ms against 453 ms) and a 100-check MCP session is 13.8x faster, with golden verdicts and counts throughout ([S7 results](developer/release-performance-results.md)). | #1011 |

Required CI compares Core alone with the TLC goldens on 29 smoke models (#840)
and ratchets the Core-only small tier nightly (#806).

## Corpus scoreboard

| Tier | Profile | Engine | Agree | Expected divergence | Refused | Budget stop | Missing golden | Disagree | Measured |
|---|---|---|---:|---:|---:|---:|---:|---:|---|
| Full Examples (185 in scope) | default | Core alone | 148 | 5 | 17 | 6 | 9 | 0 | 2026-11-06, committed baseline |
| Full Examples (185 in scope) | TLC | Core alone | 128 | 0 | 42 | 6 | 9 | 0 | 2026-11-05, committed baseline |
| Full Examples (185 in scope) | default | Reference (retired) | 146 | 4 | 13 | 13 | 9 | 0 | 2026-11-06 |
| Full Examples (185 in scope) | TLC | Reference (retired) | 150 | 0 | 15 | 11 | 9 | 0 | 2026-11-06 |
| Small (156) | default | Core | 141 | 9 | 6 | 0 | 0 | 0 | 2026-11-07, committed baseline |
| Small (156) | TLC | Core | 142 | 1 | 13 | 0 | 0 | 0 | 2026-11-09, committed baseline |
| SANY front end | | | 458/458 | | | | | 0 | 2026-11-09, committed baseline |

Full Examples selects 233 rows: 48 are out of scope (Apalache models and
similar). The full-tier runs use one worker and TLC-derived budgets capped at
900 seconds per model. The nine missing goldens are listed in #338. The Core
rows come from the committed baselines in `conformance/baselines/`
(`full-examples-core-*-w1.json` and `small-core-*-w1.json`); the small-tier
baselines are the floor the full gate and the nightly job ratchet against
([conformance snapshot](conformance.md)). The Reference rows are the retired
engine's last measurement. The two "Core alone" full-tier rows are
those committed baselines: an earlier measurement under the 900-second cap,
where six models stop on budget. They are not the release measurement. The R1 row above comes
from the later quiet-node run (#784), which caps each model at 2x the same-host
TLC time instead of 900 seconds and includes the long tier: 162 agree, 7 match
a registered divergence and 7 refuse by name. Read R1 for the current Core
coverage and these rows for the ratchet floor.

## Real-world holdout (#854)

The holdout pins seven public repositories outside tlaplus/Examples, written by
industrial teams and practitioners: microsoft/CCF, etcd-io/raft, pingcap/tla-plus,
lemmy/BlockingQueue, belaban/pluscal, Vanlightly/bookkeeper-tlaplus and
Vanlightly/kafka-tlaplus. Their manifests in `conformance/holdout/` select 30
models with the upstream `.cfg` files. Five models are out of scope: the pinned
TLC does not finish four of them within 600 seconds at one worker (both etcd
models, `BlockingQueuePoisonApple` and Kafka `kip_996/model_liveness`), and
CCF's `MCccfraft` needs a TLC JVM property for action composition. The other 25
have TLC goldens, and 26 modules have SANY goldens. These corpora are measured,
not developed against.

Core alone, in the default profile at one worker, agrees on 23 of the 25 models
(12 safety successes, 3 liveness checks that hold, 3 safety violations, 4 deadlocks and
1 evaluation error), refuses 2, and disagrees on none. All 26 modules agree
with SANY. Each refusal reason covers one model: `TLCExt!PickSuccessor`
(`BlockingQueueDebug`) and the difference of an infinite set (TiDB
`DistributedTransaction/Test3`). Measured on 2026-11-08 with `--tlc-budgets`.

Run it with `--impl dive:core --impl-profile dive --tier all` and
`--corpus holdout-ccf --corpus holdout-etcd-raft --corpus holdout-tidb
--corpus holdout-blocking-queue --corpus holdout-jgroups-pluscal
--corpus holdout-bookkeeper --corpus holdout-kafka`.

## Which Core refusals still block public models?

We rechecked nine previously refused configurations on November 22. Seven come from Examples and two from the goldened industrial holdouts. Every check used the original source, configuration and manifest mode, at one worker with no I/O grants. Eight still refused. The EWD687a animation now finds the invariant violation `InterestingBehavior` and independently validates it, matching its pinned TLC golden. This targeted recheck does not measure the rest of either corpus. [Per-model results, exact requests, pins and binary identity](../conformance/reports/2026-11-22-family-refusals.json).

<!-- refusal-counts -->

| First result on the original configuration | Configurations |
|---|---:|
| Explicit host input grant | 2 |
| Temporal VIEW quotient | 2 |
| Debugger operator PickSuccessor | 1 |
| Effectful temporal simulation predicate | 1 |
| Host clock in a specification | 1 |
| Infinite-set difference | 1 |
| Validated simulation violation | 1 |

<!-- /refusal-counts -->

**The largest measured feature gap is temporal VIEW.** `MCDistributedReplicatedLog` and `EWD998ChanID` require concrete transition and closing-edge evidence across quotient states. This affects identity, graph capture, fairness and independent replay. Count each configuration once. The other two-configuration bucket is the intended no-grant policy (D-017). `TestGraphs` requests the environment and `EWD998ChanTrace` requests a file. [VIEW tracking](https://github.com/Chelis-Lang/dive/issues/428) · [host-input contract](user/modules.md#host-inputs-and-explicit-grants).

The remaining four refusal buckets each affect one configuration. `SmokeEWD998` reaches its effectful temporal simulation property `TDSpec` after the random-operator work. `SmokeEWD998_SC` invokes the host clock at startup and would launch another process. `BlockingQueueDebug` invokes `TLCExt!PickSuccessor`. TiDB's `DistributedTransaction/Test3` needs the difference of an infinite set. These suggest separate work on sampled temporal validation, host services, debugger interaction and symbolic values. [Simulation controls](../conformance/reports/2026-11-21-simulate-conformance.json) · [SmokeEWD998 tracking](https://github.com/Chelis-Lang/dive/issues/1141).

The nine were selected from three sources. Six were excluded from the [175-model comparison](../conformance/reports/perf/2026-11-20-r2-procedure-c14687e6.json), including the animation and smoke simulation. The seventh Examples case is the host script from the full R1 selection. The remaining two are the refusals in the [25-model holdout with goldens](#real-world-holdout-854). The [13 historical non-finishes](developer/large-models.md) are a different population. Keep those results, the 235-configuration simulation survey below and individual API refusal assertions out of this denominator.

## Simulation (Examples, default profile)

`dive simulate` refuses 10 of the 235 configurations, against 110 before
the simulation design. Eleven of those it refuses or does not sample are
correct refusals; five are gaps
([histogram](../conformance/reports/2026-11-21-simulate-refusals-after.json)).
Against TLC's `-simulate` over ten seeds, 156 configurations agree and 13
more agree with TLC's simulator but not with model checking; none show a
finding ([study](../conformance/reports/2026-11-21-simulate-conformance.json)).
TLC's simulator is wrong on five configurations, three of them D-043.

## Speed

Measured on the 0.2.0 candidate `5bb88063` (#1418) under the
[R2 measurement procedure](developer/performance.md#r2-measurement-procedure):
five c7i.metal-24xl nodes (SMT off, fixed 2.4 GHz, one NUMA node, provisioned
storage), one checker at a time, one worker, seed and fingerprint 0, pinned
TLC timed fresh on the same node as Core. The 121 agreeing models where TLC
takes under 10 s are the median of six alternating repetitions (geomean
0.151x); the 48 larger models are one pass (geomean 0.229x). The geomean over
the 169 agreeing models is 0.170x TLC, and the median is 0.129x. No model is
above 1x. The four models nearest 1x:

| Model | Core / TLC |
|---|---:|
| `ewd998/EWD998Chan` | 0.87x |
| `echo/MCEcho` | 0.80x |
| `KeyValueStore/MCKVSSafetyMedium` | 0.79x |
| `GameOfLife/GameOfLife` | 0.79x |

`KeyValueStore/MCKVsnap`, 1.20x in 0.1.0, is 0.71x, and
`allocator/AllocatorImplementation`, 1.17x in 0.1.0, is 0.53x. The largest models:
`SlushProtocol/SlushLarge` 0.46x, `NanoBlockchain/MCNanoLarge` 0.41x,
`MultiPaxos-SMR/MultiPaxos_MC` 0.53x, `MultiCarElevator/ElevatorSafetyLarge`
0.53x, `ElevatorSafetyMedium` 0.58x, `bosco/bosco` 0.42x and
`bcastFolklore/bcastFolklore` 0.27x.

### Release qualification

The 0.2.0 measurements of the [release performance plan](developer/release-performance-plan.md);
methodology and tables are in [release performance results](developer/release-performance-results.md).

- D1, R2 gate: pass, geomean 0.170x TLC, none above 1x ([record](../conformance/reports/perf/2026-11-27-r2-procedure-5bb88063.json)).
- D2, memory: benchmark set 24 to 140 MiB peak RSS against 798 to 1,448 MiB for TLC at `-Xmx3g`; 64 liveness models, largest 1,929 MiB; `MCParReach` 3.1% above 0.1.0 (#1432) ([record](../conformance/reports/perf/2026-11-27-release-d2-memory-5bb88063.json)).
- D3, exact store: 1.00x to 1.25x the fingerprint store's time, 0.99x to 1.90x its peak RSS ([record](../conformance/reports/perf/2026-11-27-release-d3-store-5bb88063.json)).
- D4, worker scaling: median 8-worker speed-up 5.22x over the 28 safety models and 3.25x over all 47 (4.04x in 0.1.0, #1431); 17 of 19 temporal models 0.81x to 1.25x (#1028) ([record](../conformance/reports/perf/2026-11-27-release-d4-scaling-5bb88063.json)).
- Simulation (S4): without temporal properties 1.2x to 4.4x faster than TLC `-simulate` at 1 worker, 1.4x to 3.8x at 8; with them, `EWD840` 3.6x and 5.4x, `DijkstraMutex` 8.0x and 5.7x, `SimpleAllocator` 6.0x and 6.2x ([record](../conformance/reports/perf/2026-11-21-simulation-throughput-f685076b.json)).
- D5, startup: `dive check` of a small model 4.5 ms; bosco simulation samples 10,000 traces in 28.5 s at 46 MiB ([record](../conformance/reports/perf/2026-11-27-release-d5-throughput-5bb88063.json)).
- D6, stop latency: 7 to 43 ms after a time limit; ceilings with no overshoot ([record](../conformance/reports/perf/2026-11-27-release-d6-stop-latency-5bb88063.json)).
- D7, macOS (informational, 0.1.0 only, shared M3 laptop): geomean 0.354x TLC at 1 worker ([record](../conformance/reports/perf/2026-11-18-release-d7-macos-08a14fba.json)).

## Feature families

The 38 families group the pinned [TLC and SANY surfaces](../conformance/parity-surfaces.json). An implemented row works for the supported corpus. Each partial row below names an exercised behavior, an observable boundary and a focused control. The v0.1 column identifies the first-value families. Use the [refusal counts](#which-core-refusals-still-block-public-models) to see which boundaries affected the measured configurations.

For a Rust control, run `cargo test -p PACKAGE --test TARGET --locked` from the checkout. The link names the test target and its path identifies the package. The other links open a runnable shell control or a pinned report. Run `python3 scripts/audit-feature-families.py` to check row completeness, evidence links, the measured cohort, modes, pins and histogram. `--probe DIVE` repeats the nine requests using the conformance git cache and prints compact JSON.

| Family | Status | v0.1 | Workflow | Exercised behavior → boundary | Focused control |
|---|---|:---:|---|---|---|
| TLA+ grammar and lossless parsing | implemented | ✓ | — | — | — |
| Semantic resolution, levels and module/proof syntax | implemented | ✓ | — | — | — |
| Configuration parsing | implemented | ✓ | — | — | — |
| Model/configuration binding and startup | partial | ✓ | Bind | CFG roles, initial assumptions and their error precedence → configured `POSTCONDITION` refuses | [final_empty_properties](../crates/dive-api/tests/final_empty_properties.rs) |
| Values, equality, order, laziness and representation | partial | ✓ | Evaluate | Canonical mixed-shape sets in the default profile → some history-sensitive comparisons refuse in TLC mode; TiDB Test3 separately needs an infinite-set difference | [core_normalization](../crates/dive-api/tests/core_normalization.rs) |
| Evaluation scopes, recursion and higher-order operators | partial | ✓ | Evaluate | Lexical scopes on instance calls and primed steps → unproved query captures refuse | [core_instance_scopes](../crates/dive-api/tests/core_instance_scopes.rs) |
| Finite domains, successor enumeration and independent ENABLED | partial | ✓ | Check / replay | Independent `ENABLED` queries over proved parameter substitutions → nonidentity or hidden instance variables can refuse | [core_enabled_instance](../crates/dive-api/tests/core_enabled_instance.rs) |
| Native override discovery and installation | partial | ✓ | Bind / evaluate | Hanoi’s SHA-verified local `Bits.class` override → a changed or unknown class refuses | [bits_scope](../crates/dive-api/tests/bits_scope.rs) |
| Standard-module operators | partial | ✓ | Check / simulate | Replayable `RandomSubset` simulation draws → `TLCExt!PickSuccessor` still blocks BlockingQueueDebug | [simulate_random_ops](../crates/dive-api/tests/simulate_random_ops.rs) |
| CommunityModules operators and TLA+ fallback definitions | partial | ✓ | Evaluate | Pure `SequencesExt!ReplaceAllSubSeqs` replacements → unsupported overlapping-choice demands refuse | [core_sequence_replace](../crates/dive-api/tests/core_sequence_replace.rs) |
| Error classes and precedence | partial | ✓ | Errors / replay | Candidate errors with established source and diagnostic identities replay → a partial or unproved temporal primary can refuse | [candidate_error_traces](../crates/dive-api/tests/candidate_error_traces.rs) · [fairness_primary](../crates/dive-api/tests/fairness_primary.rs) |
| Sequential BFS safety, invariants and deadlock | implemented | ✓ | — | — | — |
| Deterministic parallel checking | implemented | ✓ | — | — | — |
| Fingerprint/exact stores and collision accounting (Core prints no collision estimate) | partial | ✓ | Store | Exact and fingerprint stores give matching concrete temporal checks → capture publishes no fingerprint collision estimate | [core_liveness](../crates/dive-api/tests/core_liveness.rs) |
| State/action constraints and action-property split | implemented | ✓ | — | — | — |
| VIEW identity and temporal lifting | partial | ✓ | Check / replay | Safety VIEW keys work in exact storage → temporal quotient closure refuses on two measured Examples models | [core_storage](../crates/dive-api/tests/core_storage.rs) |
| Symmetry declaration binding, warnings and closure | partial | ✓ | Bind | CFG symmetry declarations bind and preserve startup failure order → effectful, parameterized and state-dependent declarations refuse | [symmetry_startup](../crates/dive-api/tests/symmetry_startup.rs) |
| Symmetry reduction, concrete traces and audit | partial | ✓ | Check / replay | Safety orbit totals agree with explicit controls → the census requires complete identities and local exact safety without VIEW; temporal symmetry can use unreduced checking | [symmetry_orbit_census](../crates/dive-api/tests/symmetry_orbit_census.rs) · [symmetry_liveness](../crates/dive-api/tests/symmetry_liveness.rs) |
| Temporal language, simplification and tableaux | partial | ✓ | Temporal | Configured temporal operators and checked solutions → effectful or dynamic planning can refuse, as in the SmokeEWD998 simulation | [configured_temporal_operators](../crates/dive-api/tests/configured_temporal_operators.rs) · [liveness_solutions](../crates/dive-api/tests/liveness_solutions.rs) |
| Temporal graph construction | partial | ✓ | Temporal / store | Concrete graph capture supports exact and fingerprint stores → spilled or reclaimed temporal storage refuses | [core_liveness](../crates/dive-api/tests/core_liveness.rs) |
| WF/SF fairness | partial | ✓ | Temporal / replay | Kafka’s original fair word passes 255 fairness instances → unproved primary errors during fairness evaluation still refuse | [fairness_primary](../crates/dive-api/tests/fairness_primary.rs) · [Kafka word](../conformance/reports/perf/2026-11-21-kafka-liveness.json) |
| Liveness acceptance, counterexamples and attribution | partial | ✓ | Temporal / replay | Lasso attribution checks all properties on the reported word → the requested TLC-specific lasso choice is unavailable; another fair word can violate a different set | [liveness_attribution](../crates/dive-api/tests/liveness_attribution.rs) |
| Periodic/final/off liveness schedules | partial | | Temporal | Periodic checking can refute a property on a completed BFS level; the TLC profile defaults to final-only → explicitly requesting the bounded final/short route still refuses temporal properties | [liveness_periodic](../crates/dive-api/tests/liveness_periodic.rs) · [ordinary_final](../crates/dive-api/tests/ordinary_final.rs) |
| Simulation, generation and random replay | partial | | Simulate | Seeded finite samples and audited temporal counterexamples → truncated words can leave temporal obligations unchecked; an effectful planner can refuse | [simulate_temporal](../crates/dive-api/tests/simulate_temporal.rs) |
| DFID checking | partial | | Check | DFID certifies exhausted supported safety models → a depth cutoff yields incomplete search; temporal properties, VIEW and symmetry refuse | [core_dfid](../crates/dive-api/tests/core_dfid.rs) |
| Granted environment, file and process effects | partial | | Host services | Granted input recording and replay work for admitted reads → ungranted inputs and process execution refuse | [host_inputs](../crates/dive-api/tests/host_inputs.rs) |
| Runtime introspection and configuration services | partial | ✓ | Introspection | Simulation exposes mode-specific `TLCGet("config")` → host-clock `JavaTime` refuses at startup in SmokeEWD998_SC | [tlcget_config](../crates/dive-api/tests/tlcget_config.rs) |
| CLI/output conventions and message controls | partial | ✓ | CLI | Native and TLC exit conventions retain semantic outcomes → TLC adapter options such as `-nowarning` refuse | [exit_conventions](../crates/dive-api/tests/exit_conventions.rs) · [tlc_check](../crates/dive-cli/tests/tlc_check.rs) |
| Trace/state dumps, ALIAS and trace exploration | partial | | Trace / simulation | Saved sampled traces support ALIAS, TLC JSON and TLA output → automatic TLC TTrace exploration modules are unavailable | [simulate_trace_dir](../crates/dive-api/tests/simulate_trace_dir.rs) |
| Coverage and debugger services | partial | | Debug / coverage | A local single-worker debugger observes real evaluation → temporal debugging and logical/source coverage refuse | [debugger](../crates/dive-api/tests/debugger.rs) · [source_coverage](../crates/dive-api/tests/source_coverage.rs) |
| Bounded-memory stores, frontiers, graphs and spill | partial | | Store | Safety predecessor/frontier spilling and disk seen-state membership run → temporal disk membership refuses | [core_frontier](../crates/dive-api/tests/core_frontier.rs) · [core_disk_seen](../crates/dive-api/tests/core_disk_seen.rs) |
| Checkpoint and recovery | partial | | Recover | A fresh process can resume supported safety checkpoints → incompatible effects, identities or temporal checks refuse | [core_checkpoint](../crates/dive-api/tests/core_checkpoint.rs) |
| Distributed checking and recovery | partial | | Distribute | Authenticated workers can restart and retry uncommitted safety work → distributed liveness, VIEW and coordinator checkpoints refuse | [distributed](../crates/dive-api/tests/distributed.rs) |
| PlusCal translation and translation validation | partial | ✓ | Translate | Source flags, generated TLA+ and checked artifacts run → specification-backed translators and unsupported artifact modes refuse | [pluscal_remaining_options](../crates/dive-api/tests/pluscal_remaining_options.rs) |
| Versioned JSON reports and trace compatibility policy | implemented | ✓ | — | — | — |
| Public Rust APIs and cargo-test integration | partial | | Embed / test | External Rust consumers check, sample, translate, recover and validate traces → the API refuses source and logical coverage | [external consumer](../scripts/test-rust-consumer.sh) |
| Complete corpus, harness and release certification | partial | ✓ | Conformance | Pinned public-model goldens and holdouts grade decisions → a missing golden admits only a separate observation, even when exploration progresses | [missing-golden controls](../conformance/reports/perf/2026-11-20-no-golden-observations.json) |
| Global TLC profile and approved better defaults | partial | ✓ | Profile | Core selects the nominal TLC switches → construction-history-dependent mixed-type comparisons can refuse; default canonical comparison follows D-009 | [core_normalization](../crates/dive-api/tests/core_normalization.rs) |

Totals: 7 implemented, 31 partial, 0 missing.

The first two implemented rows cover parsing and semantic analysis, including proof syntax. A successful analysis does not establish a theorem. Source-bound finite named-property induction has its own checked obligations and scope. General authored TLAPS proofs and invariant discovery need separate controls, as does matching partial implementation event streams to a consistent TLA+ behavior. These workflows have no denominator in this TLC/SANY family table. [Finite named proofs and controls](../conformance/reports/perf/2026-11-21-named-property-proofs.json) · [authored proof and discovery](https://github.com/Chelis-Lang/dive/issues/1283) · [partial implementation traces](https://github.com/Chelis-Lang/dive/issues/1282).
