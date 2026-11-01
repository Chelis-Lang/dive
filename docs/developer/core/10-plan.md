# Plan, status and handoff log

Status: M0 passed; M1 implementation is broad but acceptance remains open.
Current source-bound findings and next steps: `m1-status.md`. Decision: ADR
0117. Umbrella issue: #498.

## 1. Milestones

| Milestone | Scope | Gate | Estimate |
|---|---|---|---|
| P0 | These documents and ADR 0117 | Docs PR merged | 1 week |
| Day 0 | #495 lock fix in the reference engine; x86 and M3 rebaseline; umbrella issue; reference engine frozen | Rebaseline JSON recorded (`09-performance-method.md`) | 2–3 days |
| M0 | Spike, default profile only. Covers handles, store, arena, hashing, a minimal plan compiler and generator for the 4 spike models, a single-thread BFS, memoization of invariants and guards, ablation switches, and an 8-thread store microbenchmark | Pass and kill criteria in section 2 | 2 weeks |
| M1 | Safety core. Covers the full binder handoff, the complete `getNextStates`, laziness and normalization semantics, error classes and replay, INSTANCE and `ENABLED`, the parallel engine, traces through the validator, `--engine`, static fallback, shadow comparison in CI, and the ablation test matrix. EWD998PCal is probed early | Small tier at or above baseline with `--engine=core`; shadow comparison clean; ≥1× TLC on 8 of 10 benchmarks at 1 worker (x86) | 8–10 weeks |
| M2 | Compat and breadth. Covers TLC intern order and runtime names in parallel; `TLCGet`/`TLCSet`, Random and FP64; VIEW; symmetry with hashing under permutation; standard and CommunityModules overrides; store compaction | Full tier ≥182 actual-core agreements and ≤5% fallback; default ≥2× TLC geometric mean and ≥1× on every benchmark at 1/8 workers; compatibility runtime/TLC ≤2 per model, investigate >1.5 (ADR 0119) | 5–7 weeks |
| M3 | Liveness through a graph-capture adapter; simulation; DFID; checkpoints; budgets; spill; disk seen set | All modes run on the core; full gate clean | 6–8 weeks |
| M4 | Core becomes the default; long tail of the corpus; x86 tuning; decision on a generated-code tier | ADR 0006 targets met; reference engine kept for validation | 3–4 weeks |

The realistic total is 6–8 months to replace the default engine. M0 shows,
after two weeks, whether the design reaches TLC-relative speed.

### Next checkpoint: trusted core and temporal capacity (2026-10-29)

The owner prioritized trusted capability and capacity, not an engine-cutover
project. The milestone estimates above are the original planning context;
this checkpoint has the following bounded deliverables and does not close
M1/M2 merely because its implementation lands. ADR 0119 records the revised
compatibility performance ceiling without weakening semantic obligations.

1. Freeze a current-source census, small-tier core/auto accounting and
   performance diagnostic. Preserve every selected fixture, refusal, stop and
   unverified result; historical reference totals are not core coverage.
2. Typed invariant/Next endpoint replay and bounded first-Init diagnostic
   receiver replay are implemented (#659/#683). Formal initial counterexamples
   stay null; `diagnostic_witness.trace` requires independent cause-word and
   fresh source/lifetime proof (ADR 0133). The former partial-Init `error_witness`
   route stays closed. Version 5 independently proves a bounded post-Next
   temporal source row (#679, ADR 0132); later rows and broader #502 remain
   explicit refusals. No unclassified oracle stream gains a deciding class.
3. The native L5 path is implemented: original targets for seen/new states,
   retained vertex/original rows, checked CSR, graph-neutral complete/prefix
   results and reports, controlled eager admission and on-demand conversion
   with bounded closed-function read reuse and unchanged eager conversion for
   every original. Ordinary core file count ceilings now use checked layouts,
   fallible growth and actual budgets (ADR 0124); explicit caller/reference
   limits stay exact. Complete accounting, remaining constructor/diagnostic
   units and original-workload capacity qualification remain (`11-liveness.md`).
4. Pure state-independent `SelectSeq` predicates under a prime and closed
   action execution are implemented. Original EWD998Chan has completed in both
   W1 profiles on source-bound development runs; the full capacity matrix is
   still required. #623 now includes bounded owned constructor histories and
   state handoff; broader producers remain open. Integrated `9fc88333` exposed
   quadratic private history indexing (#686); reviewed #687 restores exact
   SimpleRegular results without discarding histories. Its corrected-source
   qualification must replace neither failed source evidence nor later scope.
5. Qualify the integrated runtime with a full gate, the original census and
   affected ablations, and the ten-model five-repeat benchmark at actual 1/8
   workers. Keep original CoffeeCan3000/EWD998Chan/aba_asyn_byz configurations
   as primary capacity targets, with recorded comparable budgets; keep
   bcastFolklore as a separately bounded deep stress run. Never substitute a
   safety variant or a topology estimate for temporal completion.

The frozen phase 3 source `fb48dbbf`, tree `b2d90500`, includes reviewed and
CI-passing atom input-proof reuse (#709, ADR 0134) and bounded ordered
VIEW/constraint/invariant replay (#712, ADR 0135). Advance
[version-6 consumer notice](https://github.com/Chelis-Lang/oyster/issues/61#issuecomment-6558501056)
was posted before the writer landed. Its [current checkpoint](../../../conformance/reports/core/2026-11-01-phase3-checkpoint.json)
records 1,215 default / 1,187 TLC fixture-shadow agreements, with exactly ten
formerly refuted words repaired per profile and no old agreement lost. The
twenty core words validate; reference words are not separately replayed by the
ordinary shadow. All ten canonical streams remain Unclassified. TLC
`LvPeriodic` retains its known core-violation/reference-capture-resource
difference. The inherited coordinated primary-2111 class/exit admission is
tracked in #713 and excluded from the new diagnostic-proof approval.

Both Auto baselines exit 0; TLC retains all 458 passing SANY comparisons. The
one integrated full gate physically exits 0, report duration 78m07s, with 3,054
nextest passes, six source-declared ignored tests and 185 doctests. The audited
census and joined chain retain exit 1. A separately reviewed fresh-target trace
build exits 0 in 11.64s, with the forbidden packages and evaluator enumeration
absent. This supplements the owner-provided standalone-build requirement;
#714 tracks its omission as a literal command from frozen AGENTS/xtask. It is
not retroactively part of the full gate. All 21,924 local archive members are
uploaded, downloaded and verified; later documentation receives no execution
credit.

Small-payload #710 is closed unmerged after a fresh reviewed valid-negative
48-row A/B; #708 remains open. The single same-host original ABA default/W8
candidate success has a censored baseline. Fresh review approves its recovered
cell evidence after the focused controller's archival failure; it earns no
speed factor, memory savings or final-matrix acceptance.

Final-source performance has separate audited focus36 and breadth60 diagnostics,
all 96 correct rows/64 actual-core runs. All four default focus cells miss the
floor; breadth is faster on six of ten models at both widths with descriptive
throughput geometric means 1.481826/1.483856. Compatibility GameOfLife/W1 has
one breadth ratio **2.001332924**, above 2; Sailfish's above-1.5 observations
retain their investigation obligation. No formal 300-row campaign was started,
and no cohorts are pooled. Archives and performance-node/root-volume cleanup
are verified. Final original capacity is terminal at eleven successes and one
ABA TLC/W8 native atom-analysis stop at 900.076s (physical exit 5); the matrix
runner exits 1, so #650 is not accepted. Final ABA default/W8 succeeds at
833.645s and TLC/W1 at 895.382s, with only about 4.618s observed margin for the
latter. The 147-member final archive is verified. Both phase nodes/root volumes
have verified cleanup, with no active owned phase instance and no fixed-deadline
extension. Conservative compute is about $10.235, below $40, not an invoice and
excluding small storage/transfer costs. Bounded deliverables #707/#711 are manually closed after
verification. #708/#713/#714 and broader capacity/parity issues remain open.
M1/M2, #650, RC/release qualification and the reference default remain unchanged.

The historical [local qualification at runtime base `16095d43`](../../../conformance/reports/core/2026-11-01-integrated-qualification.json)
completes the full gate and audits the census/Auto results. Census findings
and TLC Auto's omitted-module exit 1 remain explicit. The separately audited
[performance diagnostics](../../../conformance/reports/perf/2026-11-01-integrated-performance.json)
retain a three-pair focus and a one-repeat breadth cohort. The focus misses the
default floor, so the formal five-repeat campaign was not started. Performance
archival and node cleanup are verified; original temporal capacity and
performance acceptance remain open. All twelve original capacity cells finish:
nine successes and three ABA atom-analysis time stops after completed graph
capture. Default/W8 loses its older completion; #650 is not accepted. Both
phase nodes/root volumes have verified cleanup. The independently audited
[capacity record](../../../conformance/reports/perf/2026-11-01-original-capacity.json)
retains the failed acceptance and prior comparison. Those historical cleanup
receipts do not establish phase 3 cleanup. The
[next bounded actions](m1-status.md#next-bounded-actions) retain the remaining
word/classification/receiver boundaries, #713, failed performance targets and
original ABA TLC/W8 completion/TLC/W1 headroom as remaining acceptance obligations.
Historical evidence is not relabeled.

The lead owns qualification/review. Three initial authors own trace trust,
explorer retention, and liveness completion/source access in separate
worktrees/targets. Move the explorer slot to bounded breadth after its
capture interface lands; release a slot for fresh review of frozen changes.
Run the local full gate and census in separate worktrees and exclusive targets.
Use at most two AWS nodes: one 32-GiB capacity VM and one metal performance
host under the quiet-host controls. Keep bounded jobs, fixed shutdown deadlines,
the $40 phase ceiling and verified external evidence preservation.

The bounded S5 normalized compound-membership index is implemented (#698).
It is worker-local, shares the memo budget and retains actual normalization/
authority admission, query demand, extensional collision verification and
miss/error checks. Scalar queries, finite-fuel analysis, TLC comparisons, raw
and unproved cases retain their original search. Integrated performance
acceptance remains open. The historical combined-source bounded profile has 28
valid stack observations: seven membership/filter paths, seven construction
projection/normalization, three frontier interning, and eleven hashing/scratch/
evaluation. These are observations, not precise time shares; remaining cost is
distributed and repeated canonical projection is also visible.

For ABA capacity, private exact-pair product lookup is implemented with separate
deterministic Cartesian final ordering (#694). Selected original-target
descriptors also remove repeated native predicate successor scans while
preserving original/stutter selection and conversion/error order (#701).
Neither change alone establishes original-workload capacity. The reduced N3/T0/F0
diagnostic finds construction larger than search for Corr/Agreement, with
tableau widths 5/2/3; it does not measure the original N4/T1/F1 cost share.
The product index keeps edges/duplicates, guards, limits, fallible growth,
budget inspections and independent witnesses exact.
The historical TLC/W8 atom-analysis stop establishes no final-source outcome.
The later #709 proof reuse retains exact input admission and original checks
when its optional one-MiB storage is unavailable. Its consumed proof counters
and one censored same-host pair do not replace final original-workload results.

The projection-reuse experiment #705 is closed unmerged: a roughly 3–4%
Sailfish gain accompanied a roughly 3–5% SimpleRegular regression. Issue #704
remains open; no loop tuning is adopted by this checkpoint.

Finer S6 keys follow only when eligibility is proved: Sailfish's reduction from
13 to six memo sites reflects intentional history safeguards, which finer keys
alone cannot remove. Broad scaling and native modes/storage remain later work.
Persistent trees remain deferred after the negative experiment. No new
code-generation tier, analyzer rewrite or default semantic divergence is
introduced by this checkpoint.

### Finite mode budget integration

Finite reference simulation and DFID now use one API-owned ADR 0107 watchdog
after loading, cooperative evaluator/execution checkpoints and final resource
sampling. Requests expose the existing time/memory settings; a stop has exit 5
and prefix progress, never a model verdict. This implements #350's bounded mode
budget feature without changing engine selection, simulation streams, DFID
completion rules or report/native trace versions. Native plan-backed mode
integration remains the next implementation step, not acceptance granted by
this budget increment.

## 2. M0 pass and kill criteria

**Pass.** All must hold.

- Distinct and generated counts and depth equal the goldens.
- At 1 worker on x86:
  - ≥1.5× TLC on MCLamportMutex and SimpleRegular;
  - ≥2.5× TLC on CoffeeCan1000BeansSafety.
- Peak RSS is no higher than the current dive.
- The interning store scales ≥6× at 8 threads.

**Kill.** Either of these stops the work:

- under 2.5× the current dive (geometric mean over the spike models);
- more than half of the gain can be reproduced by porting handles and arenas
  into the reference evaluator. That would mean a rewrite is not needed.

On a kill, amend ADR 0117 with the measurements and a revised design.

**Middle band.** If GameOfLife gains less than 1.5× while the other models
pass, continue. Laziness then becomes the critical-path item of M1.

**Not built in M0:**
- error classes beyond abort, compat order, `Print`, `TLCGet` and Random;
- INSTANCE, `ENABLED`, liveness, symmetry and VIEW;
- traces, fallback, budgets, the parallel engine and generated code.

## 3. Workstreams after M0

These run in parallel, with at most 3 heavy local agents, each in its own
worktree and target directory:

1. Values and store (`03-value-model.md`).
2. Plan compiler and laziness (`04-plan-compiler.md`, `06-memoization.md`).
3. Successor generation and engine (`05-successor-generation.md`,
   `07-engine.md`).
4. Harness, shadow comparison and bench (`08-correctness.md`,
   `09-performance-method.md`). This one is light and can be run by the lead.

**Module ownership (M1).**
- Stream 1: `handle`, `heap`, `store`, `arena`, `node`, `strings`, `convert`.
- Stream 2: `compile`, `plan`, `eval`, `memo`, `opts`.
- Stream 3: `generate`, `dive-explore`, `dive-api/src/core_engine.rs`.

A change outside a stream's own modules is kept minimal and flagged in the PR.

**Dependencies.**
- Stream 1 blocks 2 and 3 at the value interface.
- Stream 2 blocks 3 at the plan interface.
- Stream 4 is independent until the `--engine` flag lands.
- Freeze the value and plan interfaces early in M1 and record them here.

## 4. Risks

| Id | Risk | Mitigation |
|---|---|---|
| R1 | Hash-consing conflicts with observable lazy representation and normalization state | Extensional identity plus a separate lazy view (`03-value-model.md` §8) |
| R2 | Arena lifetimes break when values escape into states (lazy functions, closures, set predicates) | Deep close on intern, intern after claim, epoch-checked handles |
| R3 | Eager compilation changes which error wins | Demand-order analysis (`04-plan-compiler.md` §4); corpus-wide shadow comparison |
| R4 | Runtime-name order under compat with several workers | One worker in M1; provisional tokens with level re-execution in M2 |
| R5 | Memory growth or contention in the persistent store | Intern after claim; per-shard locks; compaction triggered by the budget |
| R6 | Cost of keeping two engines | Reference frozen to bug fixes; static fallback only; fixes land once, in the core |
| R7 | The x86 gap is larger than measured on M3 | Rebaseline on x86 on Day 0; all gates on x86 |
| R8 | The 10 ms startup target, given the reused binder | Measure in M1; move constant evaluation into the core in M2 if needed |
| R9 | Plan compile time: folding CoffeeCan's Init filter takes 0.19–0.43 s of a 0.6 s run (`m0-results.md` §6) | A fold size bound and streaming of large filters; a compile-time budget in M1 |
| R10 | Peak RSS above the reference: CoffeeCan 220 MB after the M0 fix, against 126 MB (`m0-results.md` §6) | Arena rollback for rejected fold candidates; node data inline in the store |

## 5. Decisions taken

| Date | Decision | Where |
|---|---|---|
| 2026-10-26 | Rebuild the core; freeze the old engine as reference; fingerprint store key not contract; wall-clock gate on x86 | ADR 0117 |
| 2026-10-26 | Write design documents before any code | this directory |
| 2026-10-26 | M0 x86 result: counts equal on all four models; 1.81× TLC and 5.21× the reference (geomean); kill criterion not triggered; proceed to M1. The second kill criterion (most of the gain reproducible by porting handles and arenas into the old evaluator) was not tested; the ablation attributes most of the gain to the base design rather than to any portable piece | `m0-results.md` §3.0 |
| 2026-10-26 | M0 merged as experimental `--engine=core` (#505); M1 streams 1–3 started from `main` with module ownership; owner approved the plan | this directory |
| 2026-10-26 | Where TLC is semantically wrong (stale lazy reuse, B2 and B4), the default profile evaluates soundly and `--compat=tlc` reproduces TLC; error caching is allowed in both profiles | `02-semantic-contract.md` §3a; ADR 0117 item 7 |
| 2026-10-26 | Memo tables off by default: no measured gain on x86 (0.96–1.04×); kept behind `DIVE_CORE_OPT` for later models | `06-memoization.md` (#530) |
| 2026-10-26 | Persistent function trees deferred after a negative measurement | `03-value-model.md` (#532) |
| 2026-10-27 | `--engine=auto` runs the core and falls back to the reference engine on a static refusal, before the search; it refuses result-changing options under the core. `--engine` still defaults to `reference` | `07-engine.md` (#524, #549) |
| 2026-10-27 | CI shadows the core against the reference engine on the smoke models; the full gate shadows the small tier, with sampled successor comparison | `08-correctness.md` (#534, #542) |
| 2026-10-27 | `--compat=tlc` runs on the core at any worker count: the string table is sealed after start-up names, so workers never create TLC string tokens. Small tier against the TLC-profile goldens under auto: 150 agree, 6 unsupported, 0 disagree (same as the reference engine) | `07-engine.md` (#545, #548, #549) |
| 2026-10-27 | Liveness on the core replaces only the producer: the core captures the same behaviour graph as the reference capture, and `dive-liveness` analysis, scheduling, `dive-trace` validation and the report stay the reference's code. First phase keeps the reference's capture limits; a handle-backed graph then lifts capacity to the memory budget | `11-liveness.md` |
| 2026-10-27 | M1 x86 checkpoint at `f1ed932a`: on the 5 of 10 bench models the core runs, 1.51× TLC (1 worker) and 1.27× (8 workers) geomean; counts equal the goldens. The liveness adapter is pulled forward from M3 into M1, because two bench models (EWD998PCal, BufferedRandomAccessFile) are refused only for temporal properties and the M1 gate needs 8 of 10 | §5a below; `conformance/reports/perf/2026-10-27-core-m1-x86.json` |
| 2026-10-28 | Memo tables back on by default (reverses #530): x86 A/B 1.16× at 1 worker and 1.23× at 8 (geomean, 9 models), counts identical; M0's four-model ablation had missed it. Perf PRs are judged by cycles or x86 wall time, not instruction counts alone | `06-memoization.md` §6 (#575); `conformance/reports/perf/2026-10-28-core-memo-ab-x86.json` |
| 2026-10-28 | Memo tables bounded by the run's memory budget (a sixty-fourth of `--max-memory`, 1–64 MiB, shared by the workers); eviction only drops cached values | `06-memoization.md` (#584) |
| 2026-10-28 | `ENABLED` on the core, refused inside an `INSTANCE` with substitutions; open question 8 (B3 cutoff) stays at its default, the cutoff in both profiles | `04-plan-compiler.md` (#581) |
| 2026-10-28 | `Print`, `PrintT`, `TLCSet`/`TLCGet`: an effect bit excludes them from folding, speculation and memoization; a spec that prints runs on one worker so output is in TLC's order. `RandomElement` draws through a stream trait the API supplies (no new layering edge) | `03-value-model.md` (#568, #571, #583) |

## 5a. M1 checkpoint on x86 (2026-10-27)

Bench set of 10 models, default profile, main `f1ed932a`, c7i.metal-24xl
(SMT and turbo off, CPUs 0–15), median of 5. The core ran without fallback,
so refused models have no core time. Every core run that produced a verdict
matched the goldens. Record: `conformance/reports/perf/2026-10-27-core-m1-x86.json`.

| Model | Core 1w | TLC 1w | ×TLC 1w | Core 8w | TLC 8w | ×TLC 8w | Core RSS 1w/8w (MB) |
|---|---|---|---|---|---|---|---|
| CoffeeCan1000BeansSafety | 0.94 | 2.27 | 2.40 | 0.69 | 1.67 | 2.41 | 165 / 417 |
| SimpleRegular | 2.98 | 5.46 | 1.83 | 3.68 | 2.32 | 0.63 | 56 / 154 |
| MCLamportMutex | 12.53 | 17.02 | 1.36 | 2.78 | 4.41 | 1.59 | 78 / 194 |
| MCBakery | 8.99 | 11.27 | 1.25 | 7.16 | 7.86 | 1.10 | 1026 / 1465 |
| GameOfLife | 11.73 | 12.08 | 1.03 | 2.07 | 2.60 | 1.25 | 102 / 152 |
| EWD998PCal | refused: temporal properties | 42.16 | — | — | 7.80 | — | — |
| BufferedRandomAccessFile | refused: temporal properties | 51.06 | — | — | 8.11 | — | — |
| MCbtree | refused: non-constant split argument | 40.96 | — | — | 7.97 | — | — |
| MCDisruptor_MPMC | refused: non-constant split argument | 4.45 | — | — | 1.59 | — | — |
| TLCSailfish1 | refused: INSTANCE by a non-top-level operator | 51.69 | — | — | 8.29 | — | — |

- Geomean over the five models the core runs: 1.51× TLC and 4.31× the
  reference engine at 1 worker; 1.27× TLC and 2.98× the reference at 8.
- Core scaling, 1 worker over 8: GameOfLife 5.67×, MCLamportMutex 4.51×,
  CoffeeCan 1.36×, MCBakery 1.26×, SimpleRegular 0.81× (#550).
- MCBakery's RSS is about 4× the reference engine's (#551).
- Against M0 (`m0-results.md` §3.0), one-worker times rose 10–20%
  (MCLamportMutex 10.47 → 12.53 s, GameOfLife 10.44 → 11.73 s, SimpleRegular
  2.58 → 2.98 s, CoffeeCan 0.85 → 0.94 s), the cost of M1's semantic
  coverage. Not yet attributed.
- Gate status (≥1× TLC on 8 of 10): 5 of 10 at 1 worker, 4 of 10 at 8. The
  gap is coverage first: the five refused models.

## 5b. Memo A/B and core times on x86 (2026-10-28)

Main `fdaf57a2`, same node type and method as §5a, core only (memo on vs off);
the record is `conformance/reports/perf/2026-10-28-core-memo-ab-x86.json`.
With memo on, the core's times against §5a's TLC times (a separate node of the
same type, so the ratios are indicative): MCbtree 9.4× / 7.4× (1 / 8
workers), MCDisruptor_MPMC 5.1× / 4.7×, CoffeeCan 2.3× / 2.5×, SimpleRegular
2.0× / 3.2×, MCBakery 1.8× / 1.7×, MCLamportMutex 1.8× / 1.7×, EWD998PCal
1.5× / 1.6×, GameOfLife 1.5× / 1.6×, TLCSailfish1 1.2× / 1.06×.
BufferedRandomAccessFile ran on the core only after `ENABLED` (#581). The M1
gate (≥1× TLC on 8 of 10) is therefore expected to pass, but it is decided by
one quiet run of core, reference and TLC together (handoff log, lead step 1).
SimpleRegular now scales 4.1× at 8 workers (#550 closed).

## 5c. Acceptance measurements on 2026-10-28

Current source-bound evidence and engine accounting are in `m1-status.md`
§3. The checkpoints below retain their historical sources and populations.
At `0f0c6ad1`, fresh all-on `auto` runs preserve both 156-model baselines:
144 strict default agreements plus seven expected divergences, and 150 TLC
agreements; all 458 SANY checks agree. Actual core strict agreements are
121 default and 125 TLC, with fallback counted separately.

The initial complete both-profile x86 repetition at `e2183389` records all
100 rows correct. Default beats TLC on ten of ten models at both worker counts;
TLC-compatible mode does so on eight of ten. Sailfish at one worker is
841.01 s core versus 52.02 s TLC under compat, despite identical completed
counts. This is **one repetition**, boundary-stopped from the intended
five-repeat campaign, and cannot decide acceptance. Raw runner metadata has
`quiet_window_certified=false`; no certificate is inferred from machine setup.
The compact record is
`conformance/reports/perf/2026-10-28-core-both-profile-prefix-x86.json`.
The matched five-repeat default run at fixed source `2d55317a` completes all
300 planned rows correctly. It beats TLC on ten of ten models at both worker
counts: paired geometric means 2.649× / 2.645× TLC and 7.295× / 6.747× the
reference. RSS is below TLC throughout, with several eight-worker regressions
against the reference; large-model scaling is only 1.30–4.05×. See
`conformance/reports/perf/2026-10-28-core-m1-default-x86.json` and the table in
`m1-status.md`. Later changes never relabel that executable as latest main.

The new seeded fixture census is not clean acceptance, even though the small
tier controls pass. Its trace and sampled-context findings, and the #603
hash-consing ablation reconstruction defect, are tracked separately. Do not
propose an engine-default flip from benchmark completion or aggregate auto
agreement alone.

## 6. Open questions

These are for the owner. If one blocks work, add it to `docs/open-questions.md`.
The semantic contract lists seven more, in `02-semantic-contract.md` §6.

- Should the new core's default profile also offer a 128-bit state key, as an
  option between the 64-bit fingerprint and the exact store?
- When should the old engine's crates be retired, if ever, once the core is
  the default? Retirement would cost the independent validator its separate
  evaluator.

## 7. Handoff log

Add a new entry at the top for every working session that ends with work in
progress. Give the date, branch, state and the exact next step.

- **2026-10-28 (checked membership and acceptance handoff).** Main
  `2efcb3bf` includes merged #611/#614/#616/#618 and #621, which closes
  #619 with conservative C2 normalization refusal in both profiles. Exact
  comparison/history support remains M2 work. Frozen PRE-C2 `75f0ac4f` has
  300 correct five-repeat observations at actual 1/8 workers: paired TLC/core
  geometric means are 2.620916× / 2.678114× default and 2.209552× / 2.358382×
  TLC mode. Default wins ten of ten models, TLC mode eight of ten; GameOfLife
  and Sailfish remain slower. Original quiet=false and separate host controls
  remain in `conformance/reports/perf/2026-10-28-core-checked-membership-x86.json`.
  Its full gate (40m05s, 2,730 tests) is also PRE-C2 evidence.
  Integration `e0cce6f5` separately passes fast (2,198 tests), smoke eight
  models at 1/4 workers, strict shadow 52 rows and fresh review (891,600
  differential cases, 409 native runs and 22 validated counterexample words);
  separately, both-profile Sailfish 10k prefixes matched. There is no new
  full-gate or performance-population claim.
  #620 remains `verified=false`/native-reader `Refused` diagnostic parity.
  Next: typed terminal-error/independent-word acceptance; preserve raw
  construction order, demand/history and alias lifetime before exact
  scalar/tuple normalization and raw/normalized membership, then class/E7
  widening. Persistence, `Kind::Set` and `is_norm` are not history certificates.
  Generic S5 indexing is not built. Existing INSTANCE/stdlib and L5 retained-root,
  controlled-conversion and completion work remains; native compact capture
  and measured TLC capacity are not built. M1/M2 stay open and `reference`
  remains the default. A separate post-C2 e0 one-worker Sailfish diagnostic
  has three successful arms with matching counts; it is one repetition with
  quiet=false, no eight-worker or population credit. Source qualifications
  and exact times are in `m1-status.md`.

- **2026-10-28 (engine, `l5/state-source-boundary`).** Extracted the private
  state/concrete-target source used by the existing temporal analyzer; its
  reference adapter preserves graph ownership, completion and all public
  entrypoints. Lazy-source controls cover labels, selected originals, lasso
  extraction, error demand and typed stops (issue #613; `11-liveness.md` §7.2).
  Next: expose original accepted targets through sequential and parallel graph
  events, retain all-vertex and differing-original persistent roots, then build
  controlled conversion/cache and compact completion/results. Native routing,
  bounded retention and measured TLC capacity remain unimplemented L5 work.

- **2026-10-28 (engine, `l5/borrowed-topology-source`).** First L5 dependency:
  checked borrowed CSR topology reaches the existing owned/compact product
  engines, preserving roots, duplicate edges, original positions and limits.
  The dense/captured route and checker graph retention remain unchanged. Next:
  state and original concrete-target access, retained handle roots, and
  compact-compatible completion/results before any native capacity measurement.
  L5 remains incomplete; this preparatory product API supplies no new verdict
  or capacity authority. Full-gate evidence belongs in its PR.

- **2026-10-28 (stream 1, `feat/core-sequence-replacements`).**
  - **State.** Sequence replacement now implements pure, materialized sequences
    with a uniform integer, boolean or string element class across arguments,
    through the existing library hook. Pinned fallback observations, the
    corrected partition-choice proof and bounds are in `03-value-model.md` §12.
    The absolute-position greedy implementation at `7e1195f1` was rejected by
    fresh review. All overlaps now refuse after the exact early CASE arms;
    ReplaceFirst retains its independently checked least-prefix semantics.
    Focused tests cover both profiles, worker counts, optimization switches,
    independent words and refused-case auto fallback. This is a bounded feature,
    not full sequence-operator or milestone acceptance.
  - **Next.** Overlapping All occurrences need exact normalized partition
    CHOOSE semantics. Mixed/compound/lazy sequences need fallback comparison/demand
    order; effectful arguments need repeated evaluation, not value snapshots.
    `kSubset` still needs the symbolic/startup/validator boundary already
    assessed in `03-value-model.md`; `MapThenFoldSet`/`Folds` and TLC-profile
    `AllSubSeqs` membership remain separate deliverables.

- **2026-10-28 (acceptance closeout).** Main `f1acc846` contains the scoped
  ENABLED, Init/Next, sampler, named-error trace and disabled-hashcons replay
  repairs. Full source-bound results and remaining obligations are in
  `m1-status.md`: the final fixture census is still failed acceptance; default
  matched throughput clears ten of ten, but compat, scaling, memory tradeoffs
  and broad trace certification remain. Keep the reference default. Next:
  resolve the remaining terminal-error families by evaluation site and the
  independent final-expansion strategy, then resume the existing stdlib,
  INSTANCE-substitution and L5 prerequisite plans. No graph rewrite or default
  flip follows from these measurements.

- **2026-10-28 (harness, `test/seeded-fixture-census`).** Added the repeatable
  fixture census runner described in `08-correctness.md`. Preparation at
  `e2183389` accounts for all 1,450 configs (99 parser exclusions, 1,337 selected,
  14 ambiguous skips). The original untracked/unseeded runner was not recovered;
  historical totals are explicitly unreconciled. Next: run the current fixed
  shadow harness in both profiles, preserve raw output externally, and triage
  every finding and incomplete run. Preparation adds no agreement credit.

- **2026-10-28 (harness, `fix/shadow-trace-confirmation`).** The fixture-census
  preparation exposed #590: shadow counted an independently unconfirmed trace
  as agreement and did not require missing violation words. Such rows are now
  `trace-unconfirmed` findings in ordinary and strict runs. The paired controls
  cover independently valid/refuted words, validator refusal, absent traces,
  absent documents and unreadable documents. The same PR addresses #596:
  unclassified or contradictory reports cannot earn agreement, even when equal;
  classified evaluation errors and resource stops retain their own rules.
  Next: run the workspace/shadow gates,
  then rerun the census; this change gives no retrospective measurement credit.

- **2026-10-28 (lead, handover to a new machine).** All M1 agents stopped;
  every worktree was clean and every branch pushed. No AWS nodes are running.
  - **State on `main`.** About 70 PRs merged since #505. All 10 bench models
    run on `--engine=core` with counts equal to the goldens; under
    `--engine=auto` the core also takes temporal properties (liveness through
    the reference's `dive-liveness` analysis, `11-liveness.md`), SYMMETRY
    (both profiles), VIEW, ALIAS, implied properties, `ENABLED`, `Print`,
    `RandomElement`, `TLCSet`/`TLCGet` and most standard and CommunityModules
    operators. Fixture shadow census (stream 2, 1,269 fixtures, core vs
    reference): 845 agree, 493 refused, 10 differ (6 time-limit stops, 2
    unseeded `RandomElement` runs in the census script, 2 below).
  - **Last PRs of M1 so far**, merged by the lead at handover in this order,
    each refreshed onto `main` with green CI first. If any is still open,
    finish it first.
    - #581 `ENABLED`.
    - #579 step-predicate thunks (EWD998PCal 14 → 5.5 s). The lead rebased it
      onto #581, with `Compiler::step` cleared inside `ENABLED`.
    - #584 memo cap.
    - #582 temporal properties on the core (L4). The lead rebased it onto #584,
      keeping L4's `core_engine.rs` and reapplying `memo_cap`.
    - #580 the #557 reference fix, with the full gate in its body.
  - **Lead, next.**
    1. One quiet x86 run of core, reference and TLC on the 10 bench models at
       1 and 8 workers (`scripts/aws/bench-node.sh`, §5a's arguments, with
       `--tlc`); it decides the M1 gate and checks #584's memory cap. Record a
       compact JSON and §5c here; terminate the node.
    2. Small-tier census under `--engine=auto` in both profiles, then propose
       making `auto` the default engine (`docs/open-questions.md`; M4 in the
       plan, but the evidence may support it earlier).
    3. Restart the three streams from the "Next step" sections of their docs.
  - **Stream 1 (values, stdlib), next.** `kSubset`; the sequence forms of the
    replace operators; `MapThenFoldSet` and the `Folds` module; `AllSubSeqs`
    membership under `--compat=tlc` (`CmSubSeqs` is refused); the two `Print`
    refusals (a run that prints and then ends in an error; lazy-set printing).
  - **Stream 2 (compiler), next.** `ENABLED` inside an `INSTANCE` with
    substitutions, keeping TLC's cutoff in both profiles, and D-027 for the
    default profile's cache rules in the same PR. Fix the fixture census
    script to pass `--seed`; name the six time-limit differences so the lead
    can confirm they are budget stops. No current fixture reaches the
    `Compiler::step` clear inside `ENABLED` (disabling it leaves every test
    green): add one that does, or show the path is unreachable.
  - **Stream 3 (engine), next.** Triage `DfidInitialOnly` and
    `SimExcludedInitial` (fix, or classify in the shadow); liveness phase 2
    (L5, handle-backed graph, capacity measured against TLC); SYMMETRY with
    liveness (`P4SymLive`).
  - **Tooling.** `scripts/gate-lock.sh` serializes gates and corpus runs
    across worktrees (FIFO, separate fast lane). `scripts/aws/` launches the
    quiet bench node and a Linux gate node (`remote-gate.sh`), see
    `09-performance-method.md` §2. Machine note: on the old Mac, macOS
    scanned every newly linked test binary, which stretched fast gates to
    hours; adding the terminal as a Developer Tool avoids it.

- **2026-10-27 (stream 3, liveness).**
  - **State.** Implied properties (#560), SYMMETRY (#563), ALIAS (#565),
    VIEW (#570), #550 (#562) and #551 (#566) merged. EWD998PCal runs on the
    core and matches its golden; BufferedRandomAccessFile waits for
    `ENABLED` (stream 2). Liveness design in `11-liveness.md`.
  - **Next.** PR L2 of `11-liveness.md` §9: the additive `dive-check` and
    `dive-liveness` entry points, with the full gate.

- **2026-10-27 (M1 checkpoint).**
  - **State.** 41 PRs merged since #505 (see §5). M1 x86 checkpoint
    recorded in §5a. Stream 1 is on stdlib and CommunityModules operators;
    stream 2 on non-constant split arguments and INSTANCE by non-top-level
    operators (MCbtree, MCDisruptor_MPMC, TLCSailfish1); stream 3 on a refusal
    census, then the liveness adapter (EWD998PCal, BufferedRandomAccessFile),
    then #550 and #551.
  - **Next.** Rerun the x86 bench once those five models run on the core.
    Attribute the 10–20% one-worker rise since M0.

- **2026-10-26 (M0).** Umbrella issue #498.
  - **State.** `core/m0-spike` is rebased on `main` and pushed; no PR is open.
    `dive-core` and `dive-explore` run all four spike models under
    `dive check --engine=core` (or `DIVE_ENGINE=core`), with counts equal to
    the goldens under every `DIVE_CORE_OPT` switch. Results, M3 timings
    (indicative) and ablations are in `m0-results.md`. The fast gate passes
    (nextest on the touched crates; the rest workspace-wide).
  - **Next.** The lead runs the x86 measurements of `09-performance-method.md`
    (core, reference and TLC; ablation; `store_scaling`) and records them in
    `m0-results.md` §3–5. Then decide pass or kill against §2. Watched risks
    are R9 and R10. If M0 passes, start M1 streams 1–3 from the "Next step"
    sections of `03`–`07`.

- **2026-10-26.**
  - **State.** Phase 0 documents drafted on `docs/core-reboot`. #495 fix in
    progress on `fix/host-hook-lock`.
  - **Next.** Review and merge the docs PR. Land #495. Launch the x86
    rebaseline (`09-performance-method.md` §8). Open the M0 branch
    `core/m0-spike` with `dive-core` and `dive-explore` skeletons and their
    layering rows.
