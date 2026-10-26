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
