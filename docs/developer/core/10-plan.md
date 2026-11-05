# Plan

Decision: ADR 0117, as redirected by
[ADR 0153](../../decisions/0153-core-is-the-product.md) (Core is the product).
Umbrella issue: #498. Status: [m1-status.md](m1-status.md). Release gates and
order: [RELEASE.md](../../RELEASE.md).

## 1. Milestones

The Core plan's own stages are named C0–C4, so they do not collide with the
GitHub milestones M0–M6. No dates or effort estimates are given (ADR 0153).

| Stage | Scope | Gate | State |
|---|---|---|---|
| C0 | Spike: handles, store, arena, hashing, a minimal plan compiler and generator, single-thread BFS, memoization, ablation switches | Pass and kill criteria in section 2 | Passed 2026-10-26 |
| C1 | Safety core: binder handoff, complete successor generation, laziness and normalization, error classes, INSTANCE and `ENABLED`, the parallel engine, traces through the validator, `--engine`, shadow comparison | Small-tier and CM1 shadow with no disagreement; Core-alone full-tier measurement recorded | Implemented broadly; full-tier measurement missing |
| C2 | Breadth on Core: canonical sets in the default profile (ADR 0153 §2), standard and CommunityModules operators, VIEW, symmetry, liveness at corpus scale, nominal `--compat=tlc` | Release gate R1 | Open; this is the critical path |
| C3 | Fallback removal: Core is the only checking runtime in the product path. Auto fallback and the Reference checking runtime are removed. `dive-trace` keeps its evaluator. | R1 holds; shadow gate replaced by goldens | Open |
| C4 | After v0.1: modes, storage, distribution and performance floors across the full tier | Post-v0.1 milestones | Frozen |

The earlier stages M2–M4 (compat and breadth, modes, default) are replaced by
C2–C4. Simulation, DFID, spill, checkpoints, distributed workers and DAP were
built ahead of that order. They stay merged but are frozen until after v0.1.

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
- ~~When should the old engine's crates be retired?~~ Answered by ADR 0153:
  the Reference checking runtime goes once R1 holds. `dive-eval` stays as the
  independent validator's evaluator.
