# Scoreboard

Current measurements against the [release gates](RELEASE.md). Replace rows when
a measurement is repeated; history is in git.

## Release gates

| Gate | Status | Evidence |
|---|---|---|
| R1 Coverage | **Met on main `8033ebe4`; release-candidate run pending.** Core alone, default profile, one worker, full Examples tier including the long tier, capped at 2x the same-host TLC time: 167 covered (161 agree, 6 registered expected divergences), 0 disagreements, 7 named refusals and 2 stops. The frozen Reference count on the same selection rule is 140. 48 models are out of scope and 9 have no TLC golden. | Quiet-node run in #784; #338 |
| R2 Speed | **Open until the release-candidate run.** On main `8033ebe4` the geomean is 0.205x TLC over the 167 covered models. Five models are over 1x and one, `MCKVsnap`, stopped at 2.01x; #908 has since fixed `MCKVsnap` (1.01x on a loaded host, not yet re-measured on the quiet node). | Speed table below; #784 |
| R3 Bare use | **Met on Core.** All ten smoke models resolve with no library flags; nine check and EWD998ChanID refuses by name. The CI conformance gate and the x86_64 release archive with an empty `PATH` both pass, on main `ce227b36`. | #803; #784 |
| R4 Compat exists | **Met.** `cargo xtask gate --full` passes on main `ce227b36`. In that run Core in the TLC profile ran the small tier with 142 agreements, 13 named refusals, one registered gap and no internal errors; the committed TLC-profile baseline (141 agreements, 14 refusals) is the ratchet floor and has not been refreshed from it. | #810; #784 |
| R5 Stable surface | **Met.** Command help, `--version` naming the source commit, hidden unimplemented commands (`lsp`); the help and schema-3 JSON-report golden tests pass on main `ce227b36`. | #802, #804; #784 |
| R6 Shipped | **Open.** Packaging and a release dry-run workflow exist; version is `0.0.0`, no tag. | #794 |

Required CI compares Core alone with the TLC goldens on 29 smoke models (#840)
and ratchets the Core-only small tier nightly (#806).

## Corpus scoreboard

| Tier | Profile | Engine | Agree | Expected divergence | Refused | Budget stop | Missing golden | Disagree | Measured |
|---|---|---|---:|---:|---:|---:|---:|---:|---|
| Full Examples (185 in scope) | default | Core alone | 148 | 5 | 17 | 6 | 9 | 0 | 2026-11-06, `5f3f0249` |
| Full Examples (185 in scope) | TLC | Core alone | 128 | 0 | 42 | 6 | 9 | 0 | 2026-11-05 |
| Full Examples (185 in scope) | default | Reference (retired) | 146 | 4 | 13 | 13 | 9 | 0 | 2026-11-06, `f16a52f5` |
| Full Examples (185 in scope) | TLC | Reference (retired) | 150 | 0 | 15 | 11 | 9 | 0 | 2026-11-06, `f16a52f5` |
| Small (156) | default | Core | 141 | 9 | 6 | 0 | 0 | 0 | 2026-11-07, committed baseline |
| Small (156) | TLC | Core | 141 | 1 | 14 | 0 | 0 | 0 | 2026-11-07, committed baseline |
| SANY front end | | | 458/458 | | | | | 0 | 2026-11-07, committed baseline |

Full Examples selects 233 rows: 48 are out of scope (Apalache models and
similar). The full-tier runs use one worker and TLC-derived budgets capped at
900 seconds per model. The nine missing goldens are listed in #338. The Core
rows come from the committed baselines in `conformance/baselines/`
(`full-examples-core-*-w1.json` and `small-core-*-w1.json`); the small-tier
baselines are the floor the full gate and the nightly job ratchet against
([conformance snapshot](conformance.md)). The Reference rows are the retired
engine's last measurement. The R1 and R2 rows above come from the later
quiet-node run on `8033ebe4` (#784), which caps each model at 2x the same-host
TLC time instead of 900 seconds and includes the long tier, so its counts
differ from the committed full-tier baselines.

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

## Core refusals and stops (full Examples, default profile)

Measured on main `8033ebe4` on the quiet node (#784). Every reason covers one
or two models, so work them in any order.

| Reason | Models |
|---|---|
| VIEW with liveness (deferred past v0.1 by owner ruling, 2026-11-06) | `ewd998/EWD998ChanID`, `FiniteMonotonic/MCDistributedReplicatedLog` |
| simulation with a liveness `PROPERTY` (deferred by owner ruling, 2026-11-06) | `ewd998/SmokeEWD998`, `ewd687a/EWD687a_anim` |
| `IOUtils!IOEnv` / `Json` without `--allow-io` (D-017) | `TLC/TestGraphs`, `ewd998/EWD998ChanTrace` |
| `TLC!JavaTime` | `ewd998/SmokeEWD998_SC` |
| 2x-TLC time stop in graph capture, unreduced under symmetry with liveness (D-005); fixed by #908, not yet re-measured | `KeyValueStore/MCKVsnap` |
| 24 GB memory stop in temporal analysis after 457 s (TLC takes 2181 s) | `bcastFolklore/bcastFolklore` |

`ewd998/SmokeEWD998_SC` is a host script rather than a model: it launches a
nested TLC run through `IOUtils!IOEnvExec` and names its output file with
`TLC!JavaTime`.

## Speed

Quiet node (c7i.metal-24xl, SMT off, fixed 2.4 GHz), one worker, seed and
fingerprint 0, original configurations, pinned TLC timed on the same node, on
main `8033ebe4`. The geomean over the 167 covered models is 0.205x TLC.

| Model | Core / TLC | Issue |
|---|---:|---|
| `KeyValueStore/MCKVsnap` | at least 2.01x (stop); 1.01x after #908 on a loaded host | #908 |
| `bosco/bosco` | 1.74x | #937 |
| `MisraReachability/MCReachabilityTestRandomGraphs` | 1.59x (fixed by #939, not yet re-measured) | #939 |
| `allocator/AllocatorImplementation` | 1.18x | #940 |
| `MultiCarElevator/ElevatorSafetyLarge` | 1.15x | #938 |
| `MultiCarElevator/ElevatorSafetyMedium` | 1.11x | #938 |

The long tier runs well under TLC: `MCInnerSerial`, `LCSMedium` and
`MCReachable` are each at 0.01x (#855).

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
