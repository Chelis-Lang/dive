# Scoreboard

The current numbers, and progress against the [release gates](RELEASE.md). Each
figure names when it was measured. When a measurement is repeated, replace the
row rather than adding prose. History is in git and in the reports under
`conformance/reports/`.

## Release gates

| Gate | Status | Latest evidence |
|---|---|---|
| R1 Coverage: Core alone on the full tier, default profile, 0 disagreements, at least Reference's count | **Not measured.** Core alone has never run the full tier. | — |
| R2 Speed: faster than TLC on geomean over the R1 agreeing models | Panel only: Core default profile about 2.6x TLC on 10 models (2026-10-28, `conformance/reports/perf/2026-10-28-core-m1-default-x86.json`). Full-tier timing not measured. | — |
| R3 Bare use: bundled TLAPS and CommunityModules, smoke test | **Failing.** MCPaxos and MCBakery report "cannot find module TLAPS" without `--module-path`; EWD998ChanID cannot find `SequencesExt`. | 2026-11-05 review |
| R4 Compat exists on Core | Partial. Core accepts `--compat=tlc`, and the TLC-profile small-tier shadow runs. No gap list exists yet. | — |
| R5 Stable surface | Not started. `dive check --help` errors and `lsp` is listed but unimplemented. JSON `schema_version` 3 exists without golden tests. | — |
| R6 Shipped | Not started. Version `0.0.0`, no release workflow, no tags. | — |

## Corpus scoreboard

| Tier | Profile | Engine | Agree | Expected divergence | Unsupported or refused | Budget stop | Disagree | Measured |
|---|---|---|---:|---:|---:|---:|---:|---|
| Small (156 models) | TLC | Reference | 150 | 0 | 6 | 0 | 0 | 2026-10-23 (`conformance/baselines/small-tlc-w1.json`) |
| Small (156 models) | default | Reference | 144 | 7 | 5 | 0 | 0 | 2026-10-23 (`conformance/baselines/small-dive-w1.json`) |
| Small (156 models) | TLC | Auto | 150 | 0 | 6 | 0 | 0 | 2026-11-04; 122 ran on Core, the rest fell back to Reference |
| Small (156 models) | default | Auto | 144 | 7 | 5 | 0 | 0 | 2026-11-04; 118 strict + 4 expected on Core, the rest fell back |
| Small shadow (312 rows) | default | Core vs Reference | 246 | — | 66 | — | 0 | 2026-11-04 |
| CM1 shadow (63 rows) | default | Core vs Reference | 57 | — | 6 | — | 0 | 2026-11-04 |
| Full Examples (217 models) | TLC | Reference | 182 | 0 | 16 | 10 | 0 | 2026-10-26 (`conformance/baselines/full-tlc-w1.json`, #486); 9 have no golden |
| Full Examples | default | any | — | — | — | — | — | never measured |
| Full Examples | any | Core alone | — | — | — | — | — | never measured |
| SANY front end | — | — | 458/458 | — | — | — | 0 | 2026-11-04 |

Core speed: about 2.6x TLC (geomean, default profile, 10-model panel, 2026-10-28).
A one-repeat breadth measurement gave 1.48x. The Reference engine runs at about
0.35x TLC at 1 worker. R2 will replace these with full-tier timing.

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
