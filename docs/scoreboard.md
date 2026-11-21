# Scoreboard

Current measurements against the [release gates](RELEASE.md). Replace rows when
a measurement is repeated; history is in git.

## Release gates

| Gate | Status | Evidence |
|---|---|---|
| R1 Coverage | **Met on main.** Core alone, default profile, one worker, full Examples tier including the long tier: 169 covered (162 agree, 7 registered expected divergences), 0 disagreements, 7 named refusals and no stops. The frozen historical floor on the same selection rule is 140 (see the [R1 gate](RELEASE.md#gates)). 48 models are out of scope and 9 have no TLC golden. | Quiet-node run in #784; #338 |
| R2 Speed | **Met on the 0.1.0 candidate** (`08a14fba`) under the [R2 measurement procedure](developer/performance.md#r2-measurement-procedure): one checker at a time on quiet nodes, small models as the median of six alternating repetitions. Geomean 0.195x TLC over the 169 agreeing models (fixed-set geomean 0.211x, counting the six refused models at 2x); no model above 2x. Above 1x: `MCKVsnap` 1.20x (#979) and `AllocatorImplementation` 1.17x (#940). | `conformance/reports/perf/2026-11-18-r2-procedure-08a14fba.json`; [release results](developer/release-performance-results.md) |
| R3 Bare use | **Met on Core.** All ten smoke models resolve with no library flags; nine check and EWD998ChanID refuses by name. The CI conformance gate and the x86_64 release archive with an empty `PATH` both pass on main. | #803; #784 |
| R4 Compat exists | **Met.** `cargo xtask gate --full` passes on main. In that run Core in the TLC profile ran the small tier with 142 agreements, 13 named refusals, one registered gap and no internal errors; the committed TLC-profile baseline, refreshed from a later run with the same counts, records them as the ratchet floor. | #810; #784 |
| R5 Stable surface | **Met.** Command help, `--version` naming the source commit, hidden unimplemented commands (`lsp`); the help and schema-4 JSON-report golden tests pass on main. | #802, #804; #784 |
| R6 Shipped | **Met.** v0.1.0 is released: the GitHub Release has both Linux archives with checksums, all 18 crates are on crates.io, and `cargo install dive-cli --locked` works in a clean Cargo home. | #794 |
| R7 Sail with dive | **Met.** Through Sail, a DieHard check is 38.9x faster than TLC (12.8 ms against 491 ms) and a 100-check MCP session is 12.6x faster, with golden verdicts and counts throughout ([S7 results](developer/release-performance-results.md)). | #1011 |

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

## Core refusals (full Examples, default profile)

Measured on main on the quiet node (#784). There are no stops. Every
reason covers one or two models, and each is an owner ruling or policy,
except `RandomSubset`, which Core does not implement yet.

| Reason | Models |
|---|---|
| VIEW with liveness (deferred past v0.1 by owner ruling, 2026-11-06) | `ewd998/EWD998ChanID`, `FiniteMonotonic/MCDistributedReplicatedLog` |
| `Randomization!RandomSubset` | `ewd998/SmokeEWD998` |
| `IOUtils!IOEnv` / `Json` without `--allow-io` (D-017) | `TLC/TestGraphs`, `ewd998/EWD998ChanTrace` |
| `TLC!JavaTime` | `ewd998/SmokeEWD998_SC` |

`ewd998/SmokeEWD998_SC` is a host script rather than a model: it launches a
nested TLC run through `IOUtils!IOEnvExec` and names its output file with
`TLC!JavaTime`.

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

Measured for the 0.1.0 release candidate (`08a14fba`) under the
[R2 measurement procedure](developer/performance.md#r2-measurement-procedure):
five c7i.metal-24xl nodes (SMT off, fixed 2.4 GHz, one NUMA node, provisioned
storage), one checker at a time, one worker, seed and fingerprint 0, pinned
TLC timed on the same node as Core. The 121 agreeing models where TLC takes
under 10 s are the median of six alternating repetitions (geomean 0.161x);
the 48 larger models are one pass (geomean 0.315x), with three fresh pairs
for `KeyValueStore/MCKVSSafetyMedium` (0.90x) and `ewd998/EWD998Chan`
(0.99x), which landed near 1x. The geomean over the 169 agreeing models is
0.195x TLC. No model is above 2x. Models above 1x:

| Model | Core / TLC | Issue |
|---|---:|---|
| `KeyValueStore/MCKVsnap` | 1.20x (unreduced under symmetry with liveness, D-005) | #979 |
| `allocator/AllocatorImplementation` | 1.17x | #940 |

The largest models: `SlushProtocol/SlushLarge` 0.53x, `NanoBlockchain/MCNanoLarge`
0.42x, `MultiPaxos-SMR/MultiPaxos_MC` 0.54x, `MultiCarElevator/ElevatorSafetyLarge`
0.57x, `ElevatorSafetyMedium` 0.66x, `bosco/bosco` 0.47x and
`bcastFolklore/bcastFolklore` 0.30x.

### Release qualification

The 0.1.0 measurements of the [release performance plan](developer/release-performance-plan.md);
methodology and tables are in [release performance results](developer/release-performance-results.md).

- D1, R2 gate: pass, geomean 0.195x TLC, none above 2x ([record](../conformance/reports/perf/2026-11-18-r2-procedure-08a14fba.json)).
- D2, memory: benchmark set 37 to 286 MiB peak RSS against 795 to 1,449 MiB for TLC at `-Xmx3g`; 64 liveness models, largest 4,523 MiB ([record](../conformance/reports/perf/2026-11-18-release-d2-memory-08a14fba.json)).
- D3, exact store: 1.00x to 1.30x the fingerprint store's time, 0.95x to 2.68x its peak RSS ([record](../conformance/reports/perf/2026-11-18-release-d3-store-08a14fba.json)).
- D4, worker scaling: median 8-worker speed-up 4.04x over the 47 large models; 15 of 17 temporal models 0.50x to 1.40x (#1028) ([record](../conformance/reports/perf/2026-11-18-release-d4-scaling-08a14fba.json)).
- Simulation (S4): without temporal properties 1.1x to 4.5x faster than TLC `-simulate` at 1 worker, 1.4x to 3.8x at 8; with them, `EWD840` 0.48x and `SimpleAllocator` 3.0x ([record](../conformance/reports/perf/2026-11-21-simulation-throughput-d04a3a88.json)).
- D5, startup: `dive check` of a small model 4.9 ms; bosco simulation stops at its memory budget (#1024) ([record](../conformance/reports/perf/2026-11-18-release-d5-throughput-08a14fba.json)).
- D6, stop latency: 4 to 40 ms after a time limit; ceilings with no overshoot ([record](../conformance/reports/perf/2026-11-18-release-d6-stop-latency-08a14fba.json)).
- D7, macOS (informational, shared M3 laptop): geomean 0.354x TLC at 1 worker ([record](../conformance/reports/perf/2026-11-18-release-d7-macos-08a14fba.json)).

## Feature families

This is the reference inventory of TLC and SANY surfaces
(`conformance/parity-surfaces.json`). Status describes the dive product, which
runs on Core.
- `implemented` means the feature works for the supported corpus. It does not
  mean full parity.
- **v0.1** marks the families the first-value bar depends on. The others are
  frozen until after v0.1 ([ADR 0153](decisions/0153-core-is-the-product.md)).

| Family | Status | v0.1 |
|--------|--------|:---:|
| TLA+ grammar and lossless parsing | implemented | ✓ |
| Semantic resolution, levels and module/proof syntax | implemented | ✓ |
| Configuration parsing | implemented | ✓ |
| Model/configuration binding and startup | partial | ✓ |
| Values, equality, order, laziness and representation | partial | ✓ |
| Evaluation scopes, recursion and higher-order operators | partial | ✓ |
| Finite domains, successor enumeration and independent ENABLED | partial | ✓ |
| Native override discovery and installation | partial | ✓ |
| Standard-module operators | partial | ✓ |
| CommunityModules operators and TLA+ fallback definitions | partial | ✓ |
| Error classes and precedence | partial | ✓ |
| Sequential BFS safety, invariants and deadlock | implemented | ✓ |
| Deterministic parallel checking | implemented | ✓ |
| Fingerprint/exact stores and collision accounting (Core prints no collision estimate) | partial | ✓ |
| State/action constraints and action-property split | implemented | ✓ |
| VIEW identity and temporal lifting | partial | ✓ |
| Symmetry declaration binding, warnings and closure | partial | ✓ |
| Symmetry reduction, concrete traces and audit | partial | ✓ |
| Temporal language, simplification and tableaux | partial | ✓ |
| Temporal graph construction | partial | ✓ |
| WF/SF fairness | partial | ✓ |
| Liveness acceptance, counterexamples and attribution | partial | ✓ |
| Periodic/final/off liveness schedules | partial | |
| Simulation, generation and random replay | partial | |
| DFID checking | partial | |
| Granted environment, file and process effects | partial | |
| Runtime introspection and configuration services | partial | ✓ |
| CLI/output conventions and message controls | partial | ✓ |
| Trace/state dumps, ALIAS and trace exploration | partial | |
| Coverage and debugger services | partial | |
| Bounded-memory stores, frontiers, graphs and spill | partial | |
| Checkpoint and recovery | partial | |
| Distributed checking and recovery | partial | |
| PlusCal translation and translation validation | partial | ✓ |
| Versioned JSON reports and trace compatibility policy | implemented | ✓ |
| Public Rust APIs and cargo-test integration | partial | |
| Complete corpus, harness and release certification | partial | ✓ |
| Global TLC profile and approved better defaults | partial | ✓ |

Totals: 7 implemented, 31 partial, 0 missing.
