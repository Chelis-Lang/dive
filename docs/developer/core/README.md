# The new execution core

This directory is the design and working record for the new execution core of
dive: the value model, plan compiler, evaluator, successor generation and
exploration engine. It replaces the current engine for performance while
keeping TLC's observable results. The decision is ADR 0117
(`docs/decisions/0117-core-reboot.md`).

The documents exist so that any engineer or agent can take over at any point.
Read this page, then `10-plan.md` (status and handoff log), then the documents
for the workstream you are joining.

## Status board

| Workstream | Milestone | State | Branch / PR | Doc |
|---|---|---|---|---|
| Design docs (Phase 0) | P0 | in review | `docs/core-reboot` | this directory |
| Host-hook lock (#495), old engine | Day 0 | in progress | `fix/host-hook-lock` | `01-motivation.md` |
| Rebaseline on x86 and M3 | Day 0 | not started | — | `09-performance-method.md` |
| Values and store | M0 | spike done; x86 measured (store 4.6–4.9× at 8 threads) | `core/m0-spike` | `03-value-model.md` |
| Plan compiler and laziness | M0 | spike done (spike-model subset) | `core/m0-spike` | `04-plan-compiler.md` |
| Successor generation | M0 | spike done (spike-model subset) | `core/m0-spike` | `05-successor-generation.md` |
| Memoization | M0 (ablation) | spike done; no measurable gain on x86 yet (see `m0-results.md` §3.0) | `core/m0-spike` | `06-memoization.md` |
| Engine | M0 (single thread) | spike done; `--engine=core` | `core/m0-spike` | `07-engine.md` |
| M0 results | M0 | x86 measured: 1.81× TLC, 5.21× reference (geomean); proceed to M1 | `core/m0-spike` | `m0-results.md` |
| Correctness net | M1 | not started | — | `08-correctness.md` |

The umbrella issue is #498. The M0 spike is on `core/m0-spike`; its results
are in `m0-results.md`. Update this table in every core PR.

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
