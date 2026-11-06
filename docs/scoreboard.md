# Scoreboard

Current measurements and progress against the [release gates](RELEASE.md).
Replace measurements when repeated; history stays in git. The full-tier Core
measurement below used the pinned Examples selection, one worker, 16 GiB per
model and TLC-derived time budgets capped at 900 seconds, on 2026-11-05. It
predates the pending canonical-set and recursive-function changes.

## Release gates

| Gate | Status | Latest evidence |
|---|---|---|
| R1 Coverage: Core alone, default profile, zero disagreements and at least Reference's count | **Open.** Core has 126 agreements plus two registered expected divergences among 185 in-scope Examples models, with zero disagreements. The same-source full Reference comparison is still running. Nine missing goldens remain unmeasured. | Full-tier Core measurement below; small-tier Core-only CI ratchet is active (#806). |
| R2 Speed: faster than TLC on geomean over R1 agreeing models | **Open.** Cross-host timing triage is available; it does not establish release speed. Three agreeing models exceed twice their golden TLC elapsed time and need matched confirmation. | Timing triage below. |
| R3 Bare use: bundled TLAPS and CommunityModules, smoke test | **Done.** All ten original Examples resolve without library flags; nine check successfully. EWD998ChanID retains its specifically permitted named refusal. These Reference-backed checks give no Core coverage credit. | Bundled libraries and bare-use CI smoke (#803). |
| R4 Compat exists on Core | **Done.** The TLC-profile small selection has 126 agreements and 30 named refusals, with no internal errors. This is a projection of completed full and auxiliary runs, not a new run. | Nominal switches and known gaps documented (#810); one worker. |
| R5 Stable surface | **Done.** Advertised command help works, version identifies the source commit, unimplemented commands are hidden, and CLI help and schema-3 JSON have golden tests. | #802 and #804. |
| R6 Shipped | **Open.** Packaging preparation is pending in #805. No release tag or release-candidate qualification yet. | Linux x86_64/aarch64 archives, checksums and installation/landing documentation are prepared for review. |

## Corpus scoreboard

| Tier | Profile | Engine | Agree | Expected divergence | Refused | Budget stop | Missing golden | Disagree | Measured |
|---|---|---|---:|---:|---:|---:|---:|---:|---|
| Small (156) | default | Core alone | 122 | 4 | 30 | 0 | 0 | 0 | 2026-11-05; [committed baseline](../conformance/baselines/small-core-dive-w1.json), no fallback or pending trace validation |
| Small (156) | TLC | Core alone | 126 | 0 | 30 | 0 | 0 | 0 | 2026-11-05; projection of full Examples and auxiliary runs |
| Full Examples (185 in scope) | default | Core alone | 126 | 2 | 42 | 6 | 9 | 0 | 2026-11-05 |
| Full Examples (185 in scope) | TLC | Core alone | 128 | 0 | 42 | 6 | 9 | 0 | 2026-11-05 |
| Full Examples | both | Reference | — | — | — | — | — | — | Same-source comparison running |
| Small (156), historical | TLC | Reference | 150 | 0 | 6 | 0 | 0 | 0 | 2026-10-23; [baseline](../conformance/baselines/small-tlc-w1.json) |
| Small (156), historical | default | Reference | 144 | 7 | 5 | 0 | 0 | 0 | 2026-10-23; [baseline](../conformance/baselines/small-dive-w1.json) |
| Full Examples (185 in scope), historical | TLC | Reference | 151 | 0 | 15 | 10 | 9 | 0 | 2026-10-26; Examples subset of the four-corpus measurement |
| Full four corpora (217 in scope), historical | TLC | Reference | 182 | 0 | 16 | 10 | 9 | 0 | 2026-10-26; [baseline](../conformance/baselines/full-tlc-w1.json), #486 |

Full Examples selects 233 rows: 48 are excluded and 185 are in scope, including
the nine with no golden. The small population comprises 140 Examples models,
14 probes and two configured-temporal models. The full auxiliary population
adds 32 models across probes, configured-temporal operators and validation-smoke.
The historical 182 agreements therefore span four corpora; only 151 are Examples
agreements. R1 compares identical selections on the same source. Current full
baselines remain unchanged: the Core ratchet rejects missing-oracle findings;
they must not be converted to native refusals or agreements.

The front end agrees with SANY on 458 of 458 cases (2026-11-04).

## Core full-tier refusal backlog

The default-profile histogram contains exactly 42 named refusals. Work the
largest reason bucket first (#785, #338). Reasons below are the report's exact
strings. Model IDs are relative to `specifications/` in the pinned Examples
corpus; filenames retain `.cfg`.

| Exact reason | Count | Model IDs |
|---|---:|---|
| `unsupported: a recursive function used other than applied (M1 core)` | 12 | `Prisoners/Prisoners.cfg`, `SpecifyingSystems/CachingMemory/MCWriteThroughCache.cfg`, `SpecifyingSystems/Liveness/MCLiveWriteThroughCache.cfg`, `allocator/AllocatorImplementation.cfg`, `allocator/AllocatorRefinement.cfg`, `allocator/SchedulingAllocator.cfg`, `byzpaxos/PConProof.cfg`, `byzpaxos/VoteProof.cfg`, `echo/MCEcho.cfg`, `ewd998/EWD998ChanID.cfg`, `ewd998/EWD998ChanID_export.cfg`, `transaction_commit/PaxosCommit.cfg` |
| `unsupported: quiet nested raw equality outside interval functions` | 4 | `Paxos/MCPaxos.cfg`, `PaxosHowToWinATuringAward/MCPaxosSmall.cfg`, `PaxosHowToWinATuringAward/MCPaxosTiny.cfg`, `ewd687a/MCEWD687a.cfg` |
| `unsupported: x' \in an operator application whose body builds a lazily represented set (core)` | 4 | `LeastCircularSubstring/MCLeastCircularSubstringMedium.cfg`, `LeastCircularSubstring/MCLeastCircularSubstringSmall.cfg`, `SlidingPuzzles/SlidingPuzzles.cfg`, `SlidingPuzzles/SlidingPuzzles_anim.cfg` |
| `unsupported: a set of values of different shapes: TLC's checked normalization may fail (C1, C2; the exact comparison order is not built)` | 2 | `LoopInvariance/MCQuicksort.cfg`, `SpecifyingSystems/AdvancedExamples/MCInnerSerial.cfg` |
| `unsupported: action composition (M0 core)` | 2 | `FiniteMonotonic/MCCRDT.cfg`, `FiniteMonotonic/MCReplicatedLog.cfg` |
| `unsupported: "simulation ALIAS"` | 1 | `ewd840/EWD840_anim.cfg` |
| `unsupported: "temporal properties (liveness checking): DT2"` | 1 | `ewd687a/EWD687a_anim.cfg` |
| `unsupported: "the start-up evaluation of BF: IOUtils!IOEnv requires explicit --allow-io=env (D-017)"` | 1 | `ewd998/SmokeEWD998.cfg` |
| `unsupported: SYMMETRY (M0 core engine)` | 1 | `KeyValueStore/MCKVsnap.cfg` |
| `unsupported: TLCGet("config") (this run does not provide it)` | 1 | `DieHard/MCDieHardest.cfg` |
| `unsupported: a constant substituted by a definition, inside an instance (M1 core)` | 1 | `MisraReachability/MCParReach.cfg` |
| `unsupported: a recursive operator under a prime (M0 core)` | 1 | `glowingRaccoon/product.cfg` |
| `unsupported: a std or built-in operator passed to a higher-order operator (M1 core)` | 1 | `ewd998/EWD998Small.cfg` |
| `unsupported: enumerating SUBSET of a set of more than 24 elements` | 1 | `SpanningTree/SpanTreeTest5Nodes.cfg` |
| `unsupported: splitting the next-state relation: Json!ndJsonDeserialize requires explicit --allow-io=files (D-017)` | 1 | `ewd998/EWD998ChanTrace.cfg` |
| `unsupported: temporal properties: graph capture with VIEW requires quotient edge and closing-state evidence (core)` | 1 | `FiniteMonotonic/MCDistributedReplicatedLog.cfg` |
| `unsupported: temporal properties: graph capture with random operators (core)` | 1 | `SpanningTree/SpanTreeRandom.cfg` |
| `unsupported: the split action <Init line 5, col 1 to line 5, col 17 of module MCTwoPhase> inside nested INSTANCE substitutions (M1 core)` | 1 | `TwoPhase/MCTwoPhase.cfg` |
| `unsupported: the standard operator DyadicRationals!Reduce (M0 core)` | 1 | `Huang/Huang.cfg` |
| `unsupported: the standard operator TLCExt!Trace (M0 core)` | 1 | `ewd840/EWD840_json.cfg` |
| `unsupported: the start-up evaluation of CSVFile: TLC!JavaTime (not implemented in dive's evaluator yet)` | 1 | `ewd998/SmokeEWD998_SC.cfg` |
| `unsupported: the start-up evaluation of GraphName: IOUtils!IOEnv requires explicit --allow-io=env (D-017)` | 1 | `TLC/TestGraphs.cfg` |
| `unsupported: x' \in a startup value TLC represents lazily, whose enumeration depends on its history (core)` | 1 | `LoopInvariance/MCBinarySearch.cfg` |

## Unmeasured models and declared stops

These nine IDs have **no pinned golden and are unmeasured**, reported as
`oracle-error`; no native result was obtained. They are separate from the 42
native refusals:

- `CarTalkPuzzle/CarTalkPuzzle.toolbox/Model_3/MC.cfg`
- `KeyValueStore/MCKVSSafetyLarge.cfg`
- `TLC/TestMCReachability.cfg`
- `byzpaxos/BPConProof.cfg`
- `c1cs/c1cs.cfg`
- `dag-consensus/TLCSailfish2.cfg`
- `detector_chan96/EnvironmentController.cfg`
- `diskpaxos/MC_HDiskSynod.cfg`
- `ewd998/EWD998.cfg`

These six IDs reached their declared 900-second time budget, with native exit 5.
They are budget stops, not agreements:

| Model ID | Stop phase |
|---|---|
| `MultiCarElevator/ElevatorSafetyLarge.cfg` | safety search |
| `MultiPaxos-SMR/MultiPaxos_MC.cfg` | safety search |
| `NanoBlockchain/MCNanoLarge.cfg` | safety search |
| `SlushProtocol/SlushLarge.cfg` | safety search |
| `bcastFolklore/bcastFolklore.cfg` | temporal analysis |
| `bosco/bosco.cfg` | safety search |

## Timing triage, not release speed acceptance

For the 126 agreeing Examples models with positive elapsed fields, the geometric
mean of **Core elapsed / golden TLC elapsed is 0.127**. This is cross-host triage
only: 111 TLC goldens came from macOS ARM and 15 from Linux x86_64, whereas Core
ran on Linux x86_64. Memory settings and concurrent machine load also differ.
Refusals, stops, missing goldens and expected divergences are excluded. This
number does not pass R2 or support a headline performance claim.

| Agreeing model over twice its golden TLC elapsed | Core / TLC |
|---|---:|
| `ewd998/EWD998ChanID_shiviz.cfg` | 4.34 |
| `SDP_Verification/SDP_Attack_New_Solution_Spec/MC.cfg` | 4.07 |
| `NanoBlockchain/MCNanoMedium.cfg` | 3.18 |

Confirm these on matched inputs and hardware before claiming a speed regression
or a release-wide improvement. The declared full-tier stops also remain open
performance or capacity work; timings on completed models do not resolve them.

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
