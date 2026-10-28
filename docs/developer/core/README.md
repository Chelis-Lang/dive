# The new execution core

This directory is the design and working record for the new execution core of
dive: the value model, plan compiler, evaluator, successor generation and
exploration engine. It replaces the current engine for performance while
keeping TLC's observable results. The decision is ADR 0117
(`docs/decisions/0117-core-reboot.md`).

The documents exist so that any engineer or agent can take over at any point.
Read this page, then `10-plan.md` (status and handoff log), then the documents
for the workstream you are joining.

**Start here at handover:** `m1-status.md` summarizes what M1 built, the measured results, the decisions, what is open and the next steps.

## Status board

| Workstream | Milestone | State | Branch / PR | Doc |
|---|---|---|---|---|
| Design docs (Phase 0) | P0 | done | #496 | this directory |
| Host-hook lock (#495), old engine | Day 0 | done | #497 | `01-motivation.md` |
| Rebaseline on x86 | Day 0 | done | #522 | `09-performance-method.md` |
| M0 spike | M0 | done: 1.81× TLC, 5.21× reference (x86 geomean); merged as `--engine=core` | #505 | `m0-results.md` |
| 1. Values and store | M1 | paused at handover; done: inline node data, lazy values, big ints, model values, `isNorm`, intern-token strings, lock-free intern hits, Bags, most stdlib and CommunityModules operators, Print/PrintT/ToString, RandomElement, TLCSet/TLCGet; next: `10-plan.md` §7 | `core/values-*` | `03-value-model.md` |
| 2. Plan compiler and laziness | M1 | paused at handover; done: fold bound (R9), TLC lazy values, INSTANCE, error classes, operator arguments, lazy LET, failing ASSUME, `ENABLED` (#581), memo on and bounded (#575, #584); next: `ENABLED` under INSTANCE substitutions | `core/compiler-*` | `04-plan-compiler.md`, `06-memoization.md` |
| 3. Successor generation and engine | M1 | paused at handover; done: traces, error endings, budgets, parallel engine, `--engine=auto`, `--compat=tlc` at any worker count, implied properties, SYMMETRY (both profiles), ALIAS, VIEW, VIEW+SYMMETRY, liveness L1–L4 (#572, #573, #577, #582), #550, #551; next: L5, `P4SymLive` | `core/engine-*` | `05-successor-generation.md`, `07-engine.md`, `11-liveness.md` |
| 4. Harness, shadow comparison, bench | M1 | lead; done: harness engine selection, shadow gate in CI, successor sampling, x86 checkpoint and memo A/B (`10-plan.md` §5a, §5b), AWS bench and gate scripts, reproducible seeded fixture-census runner; classified results and confirmed traces required (#590, #596); next: the M1 gate run and fresh census | — | `08-correctness.md`, `09-performance-method.md` |

The umbrella issue is #498. The M0 spike landed on `main` in #505; its results
are in `m0-results.md`. M1 streams 1–3 started 2026-10-26, each in its own
worktree, with module ownership as listed in `10-plan.md` §3. Update this table in every core PR.

## Documents

| Document | Contents |
|---|---|
| `01-motivation.md` | Measured evidence: profiles, benchmarks, why tuning the current design cannot reach ADR 0006 |
| `02-semantic-contract.md` | What the new core must reproduce, and what it may drop, each with evidence |
| `03-value-model.md` | Handles, persistent store, arenas, identity, lazy views, hashing, memory bound |
| `04-plan-compiler.md` | Binder handoff, static passes, demand-order laziness, specializations, interpreter |
| `05-successor-generation.md` | TLC's `getNextStates` rules as a specification; delta generation; dedupe before materialization |
| `06-memoization.md` | Memoized pure evaluation keyed on interned inputs: analysis, keys, soundness |
| `07-engine.md` | Breadth-first search, seen-set claims, determinism, traces, runtime names, scaling |
| `08-correctness.md` | Shadow comparison, mutation fuzzing, validator independence, fallback policy |
| `09-performance-method.md` | Benchmarks, hosts, ablations, recording format, gates |
| `10-plan.md` | Milestones, gates, workstreams, risks, decision and handoff log |
| `m1-status.md` | M1 summary at handover: what was built, results, decisions, open items, next steps |
| `11-liveness.md` | Liveness on the core: graph capture, conversion, analysis entry points, budgets, PR plan |
| `m0-results.md` | M0 spike: what was built, counts, timings, ablation, store scaling, watched risks |
| `tlc-baseline.md` | What TLC does per state, verified against the pinned TLC source |

## How to resume

1. Read the newest entry of the handoff log in `10-plan.md` and the status board
   above.
2. Check out the branch named on the status board in its own worktree, with its
   own `CARGO_TARGET_DIR` (see `AGENTS.md`).
3. Read the document for your workstream and its "Next step" section.
4. Before you stop, update that document's status and next step, the status
   board, and the handoff log. A PR that changes core behaviour without updating
   its document is incomplete.

## Rules specific to the new core

- **Contract first.** `02-semantic-contract.md` is the specification. If the
  new core needs behaviour that is not listed there, add it with evidence
  before you implement it.
- **Shadow comparison.** Every behavioural change is checked against the
  reference engine across the corpus (`08-correctness.md`).
- **Performance claims.** Every claim is a wall-clock measurement recorded with
  the method in `09-performance-method.md`. Instruction counts are diagnostic,
  not gates.
- **Unsafe code.** `unsafe` remains forbidden. A case for it needs measurements
  and an owner-approved ADR (`AGENTS.md`).
- **Frozen reference.** The reference engine (`dive-value`, `dive-eval`,
  `dive-model`, `dive-check`, `dive-sim`) receives bug fixes only.

## Glossary

- **Reference engine.** The existing evaluator and checker, frozen as oracle,
  validator evaluator and fallback.
- **Handle.** A 64-bit value word: inline scalar, or an index into a store or
  arena.
- **Persistent store.** The hash-consed, sharded store of values that are
  reachable from retained states.
- **Arena.** A per-worker bump region for transient values, reset after each
  state expansion.
- **Intern after claim.** A successor's values enter the persistent store only
  after its fingerprint wins the seen-set claim.
- **Lazy view.** The TLC representation class (interval, `SUBSET`, function
  set, and others) and normalization state of a set. It is kept beside the
  value's identity, because membership errors observe it.
- **Plan.** The compiled, slot-resolved, specialized form of the specification
  that the evaluator executes.
- **Position.** The (state, action, ordinal) key that orders successors and
  events deterministically (ADR 0015).
- **S1–S9.** The performance strategies listed in `01-motivation.md`.
