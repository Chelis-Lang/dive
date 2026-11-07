# Scoreboard

Current measurements against the [release gates](RELEASE.md). Replace rows when
a measurement is repeated; history is in git.

## Release gates

| Gate | Status | Evidence |
|---|---|---|
| R1 Coverage | **Count met, cutover run pending.** Core alone covers 153 of 185 in-scope full Examples models (148 agree, 5 expected divergences), 0 disagreements, on main `5f3f0249`. Reference covered 150 on the 2026-11-05 source. R1 is confirmed by the same-source cutover census. | [Core snapshot](../conformance/baselines/full-examples-core-dive-w1.json); #338 |
| R2 Speed | **Open.** No same-host full-tier timing yet. Known blockers over 2x TLC: SDP, NanoMedium, EWD shiviz, MCInnerSerial. | Blocker table below; #787, #820, #821, #822 |
| R3 Bare use | **Done on Reference.** All ten smoke models resolve with no library flags; nine check, EWD998ChanID refuses by name. Re-run on Core at cutover. | #803 |
| R4 Compat exists | **Done.** Core in the TLC profile runs the small tier with named refusals and no internal errors. Re-run at cutover. | #810 |
| R5 Stable surface | **Done.** Command help, source version, hidden unimplemented commands; help and schema-3 JSON goldens. | #802, #804 |
| R6 Shipped | **Open.** Packaging and a release dry-run workflow exist; version is `0.0.0`, no tag. | #794 |

Required CI compares Core alone with the TLC goldens on 29 smoke models (#840)
and ratchets the Core-only small tier nightly (#806).

## Corpus scoreboard

| Tier | Profile | Engine | Agree | Expected divergence | Refused | Budget stop | Missing golden | Disagree | Measured |
|---|---|---|---:|---:|---:|---:|---:|---:|---|
| Full Examples (185 in scope) | default | Core alone | 148 | 5 | 17 | 6 | 9 | 0 | 2026-11-06, `5f3f0249` |
| Full Examples (185 in scope) | default | Reference | 146 | 4 | 13 | 13 | 9 | 0 | 2026-11-06, `f16a52f5` |
| Full Examples (185 in scope) | TLC | Reference | 150 | 0 | 15 | 11 | 9 | 0 | 2026-11-06, `f16a52f5` |
| Small (156) | default | Core alone | 138 | 7 | 11 | 0 | 0 | 0 | 2026-11-06, `5f3f0249` plus #842 |
| SANY front end | | | 458/458 | | | | | 0 | 2026-11-04 |

Full Examples selects 233 rows: 48 are out of scope (Apalache models and
similar). The full-tier runs use one worker and TLC-derived budgets capped at
900 seconds per model. The nine missing goldens are listed in #338.

## Core refusals and stops (full Examples, default profile)

**Bold** models are covered by Reference; they are the remaining R1 gap. Every
reason covers one or two models, so work them in any order.

| Reason | Models |
|---|---|
| `TLCExt!Trace` | **`ewd840/EWD840_json`**, **`ewd998/EWD998ChanID_export`** |
| `TLCGet("config")` | **`DieHard/MCDieHardest`** |
| `SUBSET` of more than 24 elements | **`SpanningTree/SpanTreeTest5Nodes`** |
| `IOUtils!IOEnv` / `Json` without `--allow-io` (D-017) | `TLC/TestGraphs`, `ewd998/SmokeEWD998`, `ewd998/EWD998ChanTrace` |
| SYMMETRY | `KeyValueStore/MCKVsnap` |
| VIEW with liveness (deferred past v0.1 by owner ruling, 2026-11-06) | `FiniteMonotonic/MCDistributedReplicatedLog`, `ewd998/EWD998ChanID` |
| graph capture with random operators | `SpanningTree/SpanTreeRandom` |
| simulation ALIAS | `ewd840/EWD840_anim` |
| simulation with a liveness `PROPERTY` (deferred by owner ruling, 2026-11-06) | `ewd687a/EWD687a_anim` |
| 900-second stop | `MultiCarElevator/ElevatorSafetyLarge`, `MultiPaxos-SMR/MultiPaxos_MC`, `NanoBlockchain/MCNanoLarge`, `SlushProtocol/SlushLarge`, `bcastFolklore/bcastFolklore`, `SpecifyingSystems/AdvancedExamples/MCInnerSerial` |

Out of scope: `ewd998/SmokeEWD998_SC` is a host script, not a model. It
launches a nested TLC run through `IOUtils!IOEnvExec` (`java -jar ...`) and
names its output file with `TLC!JavaTime`, so it gets no refusal bucket.

Core covers ten models that Reference refuses or stops on, including
`bosco`, `ElevatorSafetyMedium`, `DijkstraMutex Safety-4-processors`,
`EWD998Chan` and `PConProof`.

## Speed blockers

Same host, one worker, seed and fingerprint 0, original configurations.

| Model | Core | Pinned TLC | Issue |
|---|---:|---:|---|
| `NanoBlockchain/MCNanoMedium` | 24.9 s with #842 (103.9 s on `5f3f0249`) | 10.8 s | #821 |
| `SDP_Verification/SDP_Attack_New_Solution_Spec/MC` | 6.9 s | 1.0 s | #820 |
| `ewd998/EWD998ChanID_shiviz` | 16.2 s | 3.1 s | #822 |
| `SpecifyingSystems/AdvancedExamples/MCInnerSerial` | stops at 900 s | 240 s | #787 |

## Feature families

This is the reference inventory of TLC and SANY surfaces
(`conformance/parity-surfaces.json`). Status describes the dive product as a
whole, whichever engine runs.
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
| Fingerprint/exact stores and collision accounting | implemented | ✓ |
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

Totals: 8 implemented, 30 partial, 0 missing.
