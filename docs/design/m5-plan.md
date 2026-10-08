# M5 plan: first verdicts

This is the implementation plan for milestone M5 (`docs/ROADMAP.md`).
- **Goal.** `dive check` reaches TLC's verdicts, counts, depth and counterexample lengths on the small-tier safety models. It does so in fingerprint and exact modes, identically at any worker count, with every counterexample independently validated.
- **Gate.** The standard conformance tier becomes a PR gate on a small subset.

**Design inputs:**

| ADR | Covers |
|---|---|
| 0012 | values |
| 0013 | IR and the `dive-model` binding stage |
| 0014 | evaluator and standard library |
| 0015 | deterministic parallel search and storage |
| 0016 | trace validation |

**Evidence.** The TLC probes are in `crates/dive-check/tests/fixtures/tlc-probes/`.

**Scope.**
- **In M5:** safety checking. That means:
  - invariants, deadlock;
  - implied initial predicates and implied actions (`PROPERTY [][A]_v` and state predicates);
  - `CONSTRAINT`, `ACTION_CONSTRAINT`, `VIEW`;
  - `ASSUME`;
  - `Print`/`Assert`/`TLCGet`/`TLCSet`;
  - fingerprint and exact stores, with disk spill.
- **Refused as `unsupported` in M5:**
  - temporal properties and fairness (`dive-liveness`);
  - `SYMMETRY`;
  - simulation;
  - `ALIAS` and `POSTCONDITION`, unless WP-D2 gets to them;
  - randomness under `--compat=tlc` until WP-C5;
  - CommunityModules operators not yet implemented.

  Each refusal names the feature.

## Prerequisites (M4, in flight)

- **`dive-sema`.** The semantic model that `dive-ir` lowers from.
- **`dive-value`.** Values, both orders and both integer modes, FP64, printing and JSON (ADR 0012).
- **`dive-config`.** The `.cfg` reader.

**ADR numbers.** Semantic analysis is ADR 0011 and values are ADR 0012; this plan's ADRs start at 0013.

## Owner decisions needed early

These are listed in `docs/open-questions.md` as questions 9–11:
- **Q11, the layering change.** The `dive-model` crate, and `dive-trace → dive-model` with `dive-eval`'s `enumerate` feature off. It blocks WP-B1 and WP-E1 from merging. Work can start on branches.
- **Q9, exploration order** in the default profile. It only affects the default-profile harness run (milestone CM4), not `--compat=tlc`.
- **Q10, `TLCGet("config").worker`** reporting the logical count, 1. It affects one corpus model, `MCDieHardest`, and only in the default profile.

## Work packages

**Size key.**

| Size | Effort for one agent |
|---|---|
| S | ≤ 1 day |
| M | 2–4 days |
| L | about a week |

**Parallelism.** Packages on different tracks can run in parallel once their dependencies have merged.

| WP | Track | What | Depends on | Size |
|---|---|---|---|---|
| **A1** | IR | `dive-ir` types and lowering from `dive-sema`: every expression kind, frames and slots, `Let`, `Subst`, recursion, origins with TLC location strings, levels, `StdOp` recognition, effects analysis. Exit: everything SANY accepts in the corpus lowers without error, and lowering is deterministic | sema | L |
| **A2** | IR | Action tree (`split`), `ActionShape` view, TLC-format action labels. Exit: unit tests against TLC's labels in the probe transcripts | A1 | M |
| **A3** | IR | TLC variable order: emulate the reversed Java `Hashtable` enumeration of the root module's context, in `dive-sema` or `dive-ir`. Exit: TLC-identical state fingerprints on the probes and a corpus sample (needs a capture of TLC fingerprints via `TLCFP` or a debug hook) | sema, value | M |
| **A4** | IR | Serialization and on-disk cache (`IR_SCHEMA`); `inline_pure_substitutions` | A1 | S |
| **B1** | Model | `dive-model` crate (after Q11): the `SpecProcessor` port. Covers constants and model values, `<-` overrides, constant precomputation, `INIT`/`NEXT`/`SPECIFICATION` extraction, `PROPERTY` classification, constraints, `VIEW`, `ALIAS`, `POSTCONDITION`, split expansion, and every `TLC_CONFIG_*` error with TLC's code. Exit: role extraction agrees with TLC on every small-tier model (compare action counts and labels from TLC's `-coverage` or the trace labels) | A2, C1 | L |
| **C1** | Eval | Plan and interpreter: all core expressions, thunks with TLC's caching rule, `CHOOSE`, quantifiers, sets through `DeferredSet`, functions, records, `EXCEPT`, recursion with depth guard, integer modes, value order, errors with origins and stacks. Split into C1a (core and logic), C1b (sets, functions, records) and C1c (laziness, `Subst`, recursion) for three agents | A1, value | L |
| **C2** | Eval | `dive_eval::next`: ports of `getInitStates`/`getNextStates`, rule by rule, with the all-assigned fast path, behind the `enumerate` feature. Exit: every probe's successor sequence and count, as unit tests | C1, A2 | L |
| **C3** | Stdlib | `dive-stdlib` core, with Java references. Naturals, Integers, Sequences, FiniteSets, Bags, Strings and TLC (`Print`, `PrintT`, `Assert`, `:>`, `@@`, `SortSeq`, `Permutations`, `ToString`, `JavaTime`, `Any`, `TLCEval`, `TLCGet`/`TLCSet` with one logical register file). Then TLCExt basics, Randomization with TLC's seeding structure, and Json `ToJson`. Parallelizable by module | C1 | L |
| **C4** | Eval | Error-class table and enumeration-order probes. Capture TLC on minimal specs for each error site (Init, Next, invariant, constraint, action constraint, implied action, assertion, overflow, fingerprint-time errors) and for each lazy set kind's enumeration order. Implement the mapping. Exit: every probe's class reproduced | C1, D2 | M |
| **C5** | Eval | `java.util.Random` port and TLC's per-state seeding. TLC-exact draws under `--compat=tlc` | C3, A3 | S |
| **D1** | Store | `dive-store` in memory: sharded `u64` fingerprint tables with exclusive-phase ownership, the canonical state codec (with the `dive-value` owners), encoded frontier blocks, the trace log. Exit: property tests (codec round trip, table semantics, min-position resolution) | value | M |
| **D2** | Check | `dive-check`, sequential: the level engine of ADR 0015 run inline. Covers initial states, events and positions, counts at E\* (§3), deadlock, constraints, invariants, implied inits and actions, trace replay, `TLCSet("exit")`, and TLC-format human output. Exit: milestone CM0 | C2, C3, B1, D1 | L |
| **D3** | Check | Parallel engine: block claiming, action-split narrow levels, per-shard resolve, phase 3, early cut, sequential-mode detection from effects, ordered `Print` buffering, chaos mode. Exit: milestone CM3 | D2 | L |
| **D4** | Store | Exact mode (`--store exact`). Exit: equals fingerprint mode on CM2 | D1, D2 | M |
| **D5** | Store | Disk spill: sorted runs with Bloom and sparse index, merge-join resolve, compaction, frontier and candidate spill, budgets. Exit: forced-spill runs (tiny budgets) equal in-memory runs on CM2; swarm-style failpoint tests | D3 | L |
| **D6** | Check | `VIEW` through the canonicalization hook; `SYMMETRY` refused; collision bound (optimistic and actual) in fingerprint mode | D2 | M |
| **E1** | Trace | `dive-trace` validator (ADR 0016) with JSON value parsing. Built and tested standalone in CI. Exit: every TLC golden counterexample in the small tier validates (positive controls) and the by-construction mutants are rejected. This can reach conformance before dive has verdicts | C1, B1 | M |
| **E2** | Harness | `--impl dive`: run dive (in-process through `dive-api`), map results to the oracle vocabulary (`EC` names, counts, depth, trace ending), and compute `trace_validated` with `dive-trace`. Add the probe directory as a local corpus (layout `probe-fixtures`) with goldens. Exit: reports for CM0–CM3 | E1, F1 | M |
| **F1** | API/CLI | `dive_api::check` and `dive check`. TLC-compatible flags (`-workers`/`--workers`, `-config`, `-deadlock`, `-seed`, `-fp`, `-nowarning`) plus `--compat=tlc`, `--store`, `--output json` (schema 1, with `options`, seed, polynomial index, store mode, collision bound, workers requested and used, compat notes), and exit codes as TLC's under `--compat=tlc` | D2 | M |
| **G1** | Bench | `dive-bench`: the survey's benchmark set against the pinned TLC, with the same method (interleaved reps, within-rep ratios, correctness checked on every run, machine recorded). Exit: a first benchmark report | F1 | M |
| **G2** | Perf | Profiling passes toward the targets below: plan fast paths, allocation, fingerprint caching, block sizing, shard count. Every change is benchmarked and passes the standard tier | G1, D3 | L, ongoing |

**Critical path:** sema → A1 → C1 → C2 → B1 → D2 → D3 → E2. Several packages run alongside it:
- A2, C3 and D1 next to C1;
- E1 next to C2 and D2;
- D4, D5 and D6 after D2/D3;
- G1 after F1.

## Conformance milestones

All runs use the pinned oracle `tlc-2026.10.06.014338` and Examples `6a7b26a`. All are compared under `--compat=tlc` unless noted.

| Milestone | Definition of done |
|---|---|
| **CM0: probes** | All 19 TLC probes in `crates/dive-check/tests/fixtures/tlc-probes/` agree at 1 worker: verdict, property, generated/distinct/queue, depth, trace length and ending, and absent counts for `InitViolation`. Plus the harness fixtures `Success`, `Deadlock`, `InvariantViolation`, `InitInvariant`, `ActionProperty`, `AssertFailure`, `EvalError`, `Overflow` (with `--int=tlc`), `Assumption`, `PrintOutput` |
| **CM1: first 20 Examples models at 1 worker** | The candidate list below agrees at 1 worker in fingerprint mode, with traces validated |
| **CM2: the small-tier safety set** | Every small-tier model that uses no M5-refused feature agrees at 1 worker. Every model that does use one is `unsupported`, naming the feature, and never `disagree`. Target: no `disagree`; at least 95 agreeing models, which is about the 99 that tla-mc accepts, minus liveness |
| **CM3: worker invariance** | CM0 and CM2 give byte-identical results (JSON minus timings) at 1, 3, 4 and 8 workers and in chaos mode, and each equals the 1-worker golden |
| **CM4: default profile** | The CM2 set under the default profile (big integers, canonical order). It agrees except for registered expected divergences, with `Overflow` flagged and the Q9 policy applied |
| **CM5: exact mode** | `--store exact` equals fingerprint mode on CM2. Forced-spill runs (D5) equal in-memory runs |

**CM1 candidate list.** The frozen list, with the replacements WP-D2 made for models whose configuration uses a refused feature, is `conformance/gates/cm1.txt`. Each was chosen as a small model with standard modules only and a safety-only configuration. The WP-E2 owner confirms each model's cfg uses no M5-refused feature before freezing the list, and replaces any that does.

| Category | Models |
|---|---|
| Violations | `DieHard/DieHard`, `DieHard/MCDieHarder`, `MissionariesAndCannibals/MissionariesAndCannibals`, `N-Queens/Queens.toolbox/FourQueens/MC`, `spanning/MC_spanning`, `tower_of_hanoi/Hanoi.toolbox/Model_1/MC` |
| Successes | `DiningPhilosophers/DiningPhilosophers`, `CigaretteSmokers/CigaretteSmokers`, `CoffeeCan/CoffeeCan100Beans`, `SpecifyingSystems/HourClock/HourClock`, `SpecifyingSystems/AsynchronousInterface/AsynchInterface`, `SpecifyingSystems/AsynchronousInterface/Channel`, `TwoPhase/MCTwoPhase`, `transaction_commit/TCommit`, `transaction_commit/TwoPhase`, `Paxos/MCVoting`, `locks_auxiliary_vars/Peterson`, `echo/MCEcho`, `chang_roberts/MCChangRoberts`, `Moving_Cat_Puzzle/CatEvenBoxes` |

## The PR conformance gate

Once CM1 passes, a PR job in `ci.yml` runs `dive-conformance run --impl dive` on a fixed subset:
- all CM0 probes and harness fixtures;
- the CM1 list;
- `MCDieHardest`, which exercises `TLCGet("config")` and must agree at every worker count;
- `GameOfLife` (heavy evaluation);
- one model of about 100k states, such as `Disruptor/MCDisruptor_MPMC`;
- one model with a constraint;
- one with `VIEW`, when D6 lands.

**How the gate runs:**
- at workers 1 and 4;
- fingerprint mode, plus exact mode on the probes;
- `--compat=tlc`.

**Budget:** under 10 minutes on the CI runner, including the harness build.
- The gate uses committed goldens only and never runs TLC.
- A `disagree` or `unsupported` on a gated model fails the PR.
- The nightly standard tier extends this to CM2 at workers 1, 4 and 8, in both stores and both profiles.

## Benchmarks to hit in M5

Measured by G1 against the pinned TLC with the survey's method (`docs/survey/competitive.md`), on interpreted dive. Each row is from ADR 0006:

| Metric | M5 target | ADR 0006 final target |
|---|---|---|
| Start-up, trivial spec | ≤ 10 ms | ≤ 10 ms |
| Throughput at 1 worker | ≥ 1.0× TLC on `CoffeeCan1000Beans`, `MCBakery`, `MCbtree`, `MCLamportMutex`, `SimpleRegular`; `GameOfLife` reported, with a plan if below 1.0× | ≥ 1.0× on every spec; ≥ 2× geometric mean |
| Throughput at 8 workers | ≥ 2× TLC geometric mean on the four specs shared with tla-mc's table | ≥ 2× geometric mean |
| Scaling 1→8, specs ≥ 0.4 M states | ≥ 3.0× | ≥ 3.5× |
| Peak RSS, `CoffeeCan1000Beans` at 8 workers | ≤ 300 MB, fingerprint mode | ≤ 300 MB |

Compiled throughput and liveness are out of M5. Every benchmark run checks that verdict and counts equal the golden. A fast wrong answer is a failure.

## Risks

- **`dive-sema` slips.** A1 can start against `dive-sema`'s current branch API and rebase, since its model mirrors SANY one-to-one.
- **The error-class table turns up TLC paths the phase model cannot express.** C4 then records them as refusals until they are understood. They are never guessed.
- **Phase-2 memory on very wide levels.** Candidate spill moves into D5, and wide-level specs get a memory budget test.
- **Q9 is decided the other way** (keep TLC's exploration order in the default profile). Exploration would then need TLC-order enumeration of canonically stored sets. ADR 0015 §7 would be revisited, and D3's design is unaffected.
