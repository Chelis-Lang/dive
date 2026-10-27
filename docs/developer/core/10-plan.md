# Plan, status and handoff log

Status: M0 spike on `core/m0-spike`, awaiting x86 measurement. Decision: ADR
0117. Umbrella issue: #498.

## 1. Milestones

| Milestone | Scope | Gate | Estimate |
|---|---|---|---|
| P0 | These documents and ADR 0117 | Docs PR merged | 1 week |
| Day 0 | #495 lock fix in the reference engine; x86 and M3 rebaseline; umbrella issue; reference engine frozen | Rebaseline JSON recorded (`09-performance-method.md`) | 2–3 days |
| M0 | Spike, default profile only. Covers handles, store, arena, hashing, a minimal plan compiler and generator for the 4 spike models, a single-thread BFS, memoization of invariants and guards, ablation switches, and an 8-thread store microbenchmark | Pass and kill criteria in section 2 | 2 weeks |
| M1 | Safety core. Covers the full binder handoff, the complete `getNextStates`, laziness and normalization semantics, error classes and replay, INSTANCE and `ENABLED`, the parallel engine, traces through the validator, `--engine`, static fallback, shadow comparison in CI, and the ablation test matrix. EWD998PCal is probed early | Small tier at or above baseline with `--engine=core`; shadow comparison clean; ≥1× TLC on 8 of 10 benchmarks at 1 worker (x86) | 8–10 weeks |
| M2 | Compat and breadth. Covers TLC intern order and runtime names in parallel; `TLCGet`/`TLCSet`, Random and FP64; VIEW; symmetry with hashing under permutation; standard and CommunityModules overrides; store compaction | Full tier ≥182 agreements with `--engine=core` and ≤5% fallback; ≥2× TLC geometric mean at 1 and 8 workers; ≥1× on every benchmark (x86) | 5–7 weeks |
| M3 | Liveness through a graph-capture adapter; simulation; DFID; checkpoints; budgets; spill; disk seen set | All modes run on the core; full gate clean | 6–8 weeks |
| M4 | Core becomes the default; long tail of the corpus; x86 tuning; decision on a generated-code tier | ADR 0006 targets met; reference engine kept for validation | 3–4 weeks |

The realistic total is 6–8 months to replace the default engine. M0 shows,
after two weeks, whether the design reaches TLC-relative speed.

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
| 2026-10-27 | M1 x86 checkpoint at `f1ed932a`: on the 5 of 10 bench models the core runs, 1.51× TLC (1 worker) and 1.27× (8 workers) geomean; counts equal the goldens. The liveness adapter is pulled forward from M3 into M1, because two bench models (EWD998PCal, BufferedRandomAccessFile) are refused only for temporal properties and the M1 gate needs 8 of 10 | §5a below; `conformance/reports/perf/2026-10-27-core-m1-x86.json` |

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
