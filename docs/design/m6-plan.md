# M6 plan: liveness, fairness and symmetry

This is the implementation plan for milestone M6 (`docs/ROADMAP.md`, "Checking features": liveness and fairness, then symmetry).

- **Goal.** `dive check` reaches TLC's verdicts, counts and error classes on every small-tier model with temporal properties or `SYMMETRY`. It does so identically at any worker count and in both stores, with every liveness counterexample independently validated (lasso, fairness and property).
- **Exit.** The 53 small-tier models refused today for temporal properties and the 11 refused for `SYMMETRY` all agree under `--compat=tlc`. The PR gate covers liveness and symmetry.

**Design inputs:**

| ADR | Covers |
|---|---|
| 0017 | TLC's liveness semantics, and what conformance compares |
| 0018 | dive's liveness architecture |
| 0019 | symmetry |
| 0015, 0016 | the search engine and the trace validator, which M6 extends |

**Evidence.** The TLC probes are in `crates/dive-liveness/tests/fixtures/tlc-probes/` (43 probes, with `-lncheck default`, multi-worker and `-fp` variants recorded in the README).

**Scope.**
- **In M6:**
  - temporal `PROPERTY`s with `WF`/`SF` fairness, `~>`, `[]<>`, `<>[]`, tableaux, and mixed safety and liveness properties;
  - `-lncheck final|default|off|seq*`;
  - liveness under `CONSTRAINT`, `ACTION_CONSTRAINT` and `VIEW`;
  - `SYMMETRY` for safety;
  - symmetry with liveness: unreduced checking by default (D-005), with explicit
    refusal and TLC's behavior under `--symmetry-liveness=tlc`;
  - lasso validation in `dive-trace`.
- **Not in M6:**
  - simulation-mode liveness;
  - checkpoint and resume of the behaviour graph;
  - a parallel SCC algorithm, unless WP-L3's measurements demand one.

**The counts in this plan come from:**
- the report `conformance/reports/20261009T005202798-dive-small.json`: 140 small-tier models, of which 69 agree, 2 disagree (D-010) and 69 are unsupported. 53 of the unsupported are refused for temporal properties; one of those, `SDP_Attack_Spec`, is also refused for `Bitwise!Xor`. 11 are refused for `SYMMETRY`, and none of those also checks liveness;
- the goldens: 2 of the 53 are liveness violations (`MCDistributedReplicatedLog`, `MCRealTimeHourClock`) and 2 are safety violations found before liveness runs (`QueensPluscal` `FourQueens`, `SDP_Attack_Spec`).

## Approved defaults

Questions 14–17 are resolved in `docs/open-questions.md`.

- **Q14:** `--lasso=short` is the default: shortest prefix and a deterministic
  short cycle. TLC's choice remains under `--lasso=tlc` and `--compat=tlc`.
- **Q15:** `--symmetry-liveness=unreduced` disables symmetry for the entire
  run and reports that choice. Keep the explicit `refuse` and `tlc` modes.
- **Q16:** reject non-bijective symmetry declarations by default, preserving
  TLC's permissive behavior in compat mode.
- **Q17:** the `--check-symmetry` model audit is opt-in.

## Implementation status

L4's independent temporal word kernel is implemented for pure boolean
combinations, `[]`, `<>`, `~>` and state/action atoms. Its tests cover every
suffix of all boolean words up to four positions and six pinned TLC liveness
words. Named pure periodic counterexamples now validate independently,
including behavior/closing-edge checks and the complete violated-property
set. Eight pinned words validate, including recurring-action fairness
certificates; closing-edge/property/constraint mutations are rejected.
Fairness without a recurring non-stuttering action, independent
`ENABLED`, VIEW/symmetry closing evidence and harness integration remain
outstanding. L3 now provides validated SCC decomposition
and accepting-lasso search over supplied product graphs, including unrestricted
prefix reachability, all acceptance visits and deterministic witnesses. TLC-exact formula/tableau planning,
behavior-graph capture, concrete-state projection and property
attribution remain outstanding. L1 now has a bounded, flat boolean/always/
eventually formula kernel with negation normalization and generalized Buchi
tableaux over supplied state/action atoms. Exhaustive short-word tests compare
the accepted language with direct temporal semantics; an omitted promise is
caught by a planted-bug control. Bound pure predicates now lower to that
kernel with retained state/action scopes, source-located stable diagnostics,
seven pinned property-word controls and exhaustive short-word comparisons.
Fairness/ENABLED expansion, parameter/dynamic contexts, TLC's DNF/classification/
order and tableau numbering remain outstanding.
L2 and L5–L10 remain planned.

## Work packages

**Size key:** S ≤ 1 day; M 2–4 days; L about a week.

**Parallelism.** Packages on different tracks can run in parallel once their dependencies have merged. Within the liveness track, L1, L4 and L5 have no dependency on each other and can start on day one.

| WP | Track | What | Depends on | Size |
|---|---|---|---|---|
| **L1** | Liveness | Formula pipeline in `dive-liveness::formula` (ADR 0018 §2). Port `astToLive`, `pushNeg`, simplification, DNF with TLC's orders, classification into EA, AE-state, AE-action and tableau formula, grouping into OoS, and `TBGraph`/`TBPar` with TLC's node numbering. Includes errors 2213, 2214 and 2253 (exit 77), warnings, and the "N branches" message. Exit: unit tests on every probe's formula (number of branches as TLC prints them), `LvTautology` (not a tautology), `LvDoubleNeg`; the binder-to-plan path on all 53 models without a refusal | — | L |
| **L2** | Liveness | Behaviour graph during the search (ADR 0018 §3–§4). `LiveHooks` in `dive-check`; `StateSet::with_ids` in `dive-store`; atom evaluation in phase 1 (state, pair, stutter pair, successor tableau consistency, initial consistency); phase-L events (errors) and initial-state consistency events; the append-only CSR graph with interned bit patterns; incremental tableau reachability and finite-prefix detection (exit 12); `VIEW` side table and duplicate-edge rule. Exit: graph bytes identical at 1–64 workers and under chaos on every probe; `LvEvalError`, `LvEvalErrorLater` and the finite-prefix probe reproduce TLC's class, counts and trace | L1 | L |
| **L3** | Liveness | Default lasso search, `--lasso=short` (ADR 0018 §5). Parallel (OoS, PEM) tasks; EA pruning; SCC decomposition; the accepting test; the declarative choice; canonical prefix and cycle; stuttering and back-to endings; property attribution (`evalOnLasso` port). `-lncheck final` and `off`. TLC-format human output and JSON. Exit: milestone LM0 (verdicts, properties, counts) on every probe; ADR 0018 §5's table confirmed | L2 | L |
| **L4** | Trace | Lasso validation in `dive-trace` (ADR 0018 §8): format additions; the temporal evaluator on lassos; fairness and property checks; `ENABLED` witnesses; refusal by name of false `ENABLED` until L5. In the harness: lasso validation of dive's output, positive controls (the 2 goldens and every probe `trace.json`), lasso mutants, the `agree-lasso-unmatched` outcome and the property-set rule (ADR 0017 §2). Exit: all TLC liveness counterexamples validate or are refused by name, none is invalid; all mutants are rejected | — (format agreed with L3) | M |
| **L5** | Trace | `dive_trace::exists_successor`: an independent successor-existence search written from TLA+ semantics, not from `dive_eval::next`. It decides false `ENABLED`, and so fair stuttering endings and deadlock validation (closing ADR 0016 amendment 1's deferred option). Builds without `enumerate`. Exit: agrees with the checker's `ENABLED` and deadlock on every probe and small-tier deadlock golden; refuses by name what it cannot decide | — | M |
| **L6** | Liveness | `--lasso=tlc` (ADR 0018 §6). TLC `SetOfStates` order, with TLC fingerprints and the capacity history; tableau edge nesting and the recursive variant's order; ports of `checkSccs`, `checkComponent`, `getPath` (both variants), `dfsPostFix`, `bfsPostFix`, and state rebuilding. Exit: milestone LM3 | L3 | L |
| **L7** | Liveness | `-lncheck default` at level boundaries (ADR 0018 §7): the doubling schedule, "current state space" checks over expanded nodes only, counts at the stop. Exit: deterministic across workers and chaos on `LvPeriodic`; the verdict equals TLC's; counts equal the level-boundary formula | L3 | M |
| **L8** | Liveness | Memory: graph budget, edge-block spill through the ADR 0015 §5 spill layer, paged reads for the searches, spilled working arrays, resource-limit refusal. Exit: forced-spill runs (tiny budgets) give byte-identical results on LM2 | L3, D5 | M |
| **L9** | Liveness | Sequential mode for liveness atoms with side effects, and TLC's atom evaluation order under `--compat=tlc` (ADR 0018 §10, phase L). Exit: a probe with `TLCGet` or `TLCSet` in a property, against TLC | L2 | S |
| **L10** | Perf | Benchmarks and profiling toward ADR 0018 §13: `ElevatorLivenessMedium`, the overhead ratio against safety-only runs, scaling; pattern-interning and edge-width choices. Exit: a benchmark report in `crates/dive-bench` | L3 | M, ongoing |
| **S1** | Symmetry | Binding: port of `Tool.getSymmetryPerms` and `MVPerms.permutationSubgroup` in `dive-model`. Every error in TLC's order, closure to a group, the too-small heuristic that disables reduction (warning 2300), the "unused set" warning. Exit: one probe per error and warning captured against TLC; `MCPaxos` binds with reduction off | — | M |
| **S2** | Symmetry | Canonicalisation in `dive-check`'s key hook (ADR 0019 §1–§2). TLC-order minimum under `--compat=tlc`, the run's order in the default profile; early exit; lazy permutation; `VIEW` after symmetry; the D-010 flag extended to complete searches with `SYMMETRY`; validation of symmetric counterexamples. Exit: milestone SM1 | S1 | M |
| **S3** | Symmetry | Symmetry with liveness (ADR 0019 §4, ADR 0018 §9). unreduced checking by default (reduction disabled for the whole run), explicit `DIVE-L0010` refusal, and `--symmetry-liveness=tlc` quotient construction; `ValidModuloSymmetry` validation. Exit: milestone SM3 | S2, L3 (L6 for exact lassos) | M |
| **S4** | Symmetry | Performance: the sort-based canonical form for the default profile, and benchmarks on `FastPaxos` (144 permutations) and `MultiPaxos_MC_small`. Exit: equal results to S2 on SM1, with the speed-up measured | S2 | M |
| **S5** | Symmetry | `--check-symmetry` audit and the non-bijective warning or refusal, after Q16 and Q17 | S2 | S |
| **H1** | Harness | Liveness probes as a local corpus with goldens (layout `probe-fixtures`, as WP-E2 did for M5's probes). Golden fields for the named property set, the ending and `back_to`, and the number of branches. The `agree-lasso-unmatched` outcome. `--impl dive` passes `--lncheck final` and the `--lasso` mode. `dev capture --lncheck/--workers/--fp` (done in this design PR). Exit: harness reports for LM0 | L4 for validation | S |
| **H2** | Harness | Gate additions (below), and the registry entry D-013 if Q14 is answered `short` | LM1 | S |

**Critical path:** L1 → L2 → L3 → (LM1) → L6 → (LM3).
- L4 and L5 run next to L1–L3.
- S1 and S2 are independent of the liveness track and can run in parallel from the start.
- S3 joins after L3.

## Conformance milestones

All runs use the pinned oracle `tlc-2026.10.06.014338` and Examples `6a7b26a`. All compare under `--compat=tlc`, at workers 1, 4 and 8, in fingerprint mode, unless noted. "Agree" for a liveness violation means verdict, error class, exit code, counts and a validated lasso. It also means the named property set and the lasso's length, ending and back-to index once `--lasso=tlc` exists (LM3). Before that, such a model is `agree-lasso-unmatched`, which the gate treats as not agreeing.

| Milestone | Definition of done |
|---|---|
| **LM0: probes** | Every `Lv*` probe in `crates/dive-liveness/tests/fixtures/tlc-probes/` reproduces TLC's verdict, error class, exit code and statistics. Every lasso validates (or is refused by name for a false `ENABLED` before L5). Includes the surprising cases: `LvConstraint` success, `LvTautology` success, `LvMixedState` exit 12 as an invariant, `LvMixedAction` exit 13 as an action property, `LvTwoProps` naming both properties, `LvEvalError`'s `GENERAL` wrapping |
| **LM1: first liveness Examples** | Fifteen models at 1, 4 and 8 workers. **Violations:** `FiniteMonotonic/MCDistributedReplicatedLog`, `SpecifyingSystems/RealTime/MCRealTimeHourClock` (verdict, property, counts; lasso validated; `agree-lasso-unmatched` accepted at this milestone only). **Safety violation found before liveness:** `N-Queens/QueensPluscal.toolbox/FourQueens/MC`. **Successes:** `DiningPhilosophers`, `Prisoners_Single_Switch/PrisonerSolo`, `PrisonerSoloLightUnknown`, `Prisoner`, `PrisonerLightUnknown`, `CoffeeCan/CoffeeCan100Beans`, `ewd840/SyncTerminationDetection`, `Prisoners/Prisoners`, `Moving_Cat_Puzzle/CatEvenBoxes`, `CatOddBoxes`, `chang_roberts/MCChangRoberts`, `SpecifyingSystems/Liveness/LiveHourClock`. Each TLC golden takes under 2 s |
| **LM2: all temporal models** | All 53 agree, at workers 1, 4 and 8, in chaos mode, and in both stores. `SDP_Attack_Spec` needs `Bitwise!Xor` (outside M6); until then it stays `unsupported` naming only that. Heaviest: `CoffeeCan1000Beans` (501,500 states), `SpanTreeRandom` (79,550 states, seeded randomness), `bcastByz`, `ElevatorLivenessMedium` |
| **LM3: TLC's lassos** | With `--lasso=tlc`: every probe and both goldens match TLC's length, ending, back-to index and named property set. The six models whose TLC counterexample depends on `-fp` (ADR 0017 §10) also match at `--fp 1, 2, 3, 7, 42`, against the transcripts recorded for this plan |
| **LM4: default profile** | The LM2 set in the default profile. Agrees, except registered expected divergences. Lasso differences are under D-013 if Q14 says `short`; at most the two liveness violations are affected |
| **LM5: modes** | `-lncheck default` (L7) and forced-spill runs (L8) on the LM2 set: verdicts equal; under `final`, results are byte-identical to in-memory runs |
| **SM1: symmetry, safety** | Probes `SymCounts`, `SymCountsNone`, `SymAsym`, `SymAsymLate`, `SymOrder`, `SymTwoSets`, `SymView`, `SymViolation`, and S1's error and warning probes. The 11 small-tier models: `MCCheckpointCoordinationFailure` (violation, trace 12), `MCFlashWithMutex`, `MCGermanCMPWithMutex`, `MCGermanControl`, `MCGermanData`, `MultiPaxos_MC_small`, `MCPaxos` (no reduction, warning 2300), `MCVoting`, `FastPaxos`, `Paxos`, `BufferedRandomAccessFile`. At 1, 4 and 8 workers, chaos, both stores |
| **SM2: symmetry, default profile** | The SM1 set in the default profile: equal counts, with the D-010 flag raised where ADR 0019 §1 says |
| **SM3: symmetry with liveness** | Default: `SymLiveWarn`, `SymLiveMissed` and `SymLiveSymProp` match unreduced oracle verdicts; reduction is disabled for the whole run and reported. Explicit `refuse` mode reports `DIVE-L0010`. Under `--compat=tlc`: TLC's verdicts (success, success, violation), warning 2279 and counts; the spurious lasso is reported with `InvalidUnderSymmetry`. Deep tier: `KeyValueStore/MCKVsnap` |

## Gate additions

The `Conformance gate` (AGENTS.md) runs `conformance/gates/cm1.txt` at workers 1, 4 and 8. M6 adds:

- **Probes**, after LM0: every `Lv*` and `Sym*` probe, through the H1 local corpus, at 1 and 4 workers. Exact mode on the probes.
- **`conformance/gates/lm1.txt`**, after LM1: the fifteen LM1 models. Each golden takes under 2 s, so the list fits the gate's 10-minute budget alongside CM1.
- **`conformance/gates/sm1.txt`**, after SM1: the fast symmetry models, `MCGermanControl`, `MCGermanData`, `MCGermanCMPWithMutex`, `MCFlashWithMutex`, `MCVoting`, `MCPaxos`, `SimplifiedFastPaxos/Paxos` and `BufferedRandomAccessFile` (each under 4,000 distinct states).
- **Chaos.** One chaos seed per gated liveness model. Liveness adds a sequential per-level step and an after-search phase, so it is where a scheduling leak would show.
- **The gate's acceptance rule** stays "only `agree`". `agree-lasso-unmatched` and `agree-pending-trace-validation` fail it. LM1's two violations enter the gate after LM3, or after Q14 if it is answered `short` and D-013 is registered for the default profile.

The nightly standard tier runs LM2, SM1 and SM3 at workers 1, 4 and 8, in both stores and both profiles, with `--lasso=tlc` and `--lasso=short`.

## Benchmarks to hit in M6

Measured by L10 and S4 against the pinned TLC, with the survey's method:

| Metric | M6 target |
|---|---|
| `ElevatorLivenessMedium`, 8 workers | ≤ 0.5 s (ADR 0006); TLC 5.9 s |
| Liveness overhead over the same model's safety-only run, 1 worker | ≤ 1.5× time; ≤ 32 bytes per state plus 8 per edge |
| `CoffeeCan1000Beans` (liveness), peak RSS at 8 workers | ≤ 300 MB (ADR 0006's figure for fingerprint mode, now including the graph) |
| Symmetry, `FastPaxos` and `MultiPaxos_MC_small`, 1 worker | ≥ 1.0× TLC; ≥ 2× with S4's canonical form in the default profile |

## Risks

- **`--lasso=tlc` is a deep port.** It depends on `SetOfStates`' hashing and capacity history, and on `LiveWorker`'s details. Its fixture is the `-fp` series, which exercises different hash layouts on the same specs. If it slips, LM2 still completes with `agree-lasso-unmatched` for two models, and Q14 decides how the default profile is judged.
- **False `ENABLED` in validation.** Until L5, fair stuttering endings cannot be validated. Of the goldens, only `MCRealTimeHourClock` ends in stuttering, and it has no fairness (2257), so the gap matters mainly for probes.
- **Atom evaluation order.** Errors in two different liveness predicates on one state would be reported in TLC's set order only under `--lasso=tlc`'s ordering. This is flagged, and no corpus model is known to hit it.
- **Graph memory on the deep tier.** L8's spill is required before deep-tier liveness models run. Until then, they end as a resource limit rather than run unbounded.
- **Symmetry cost.** Unreduced groups of 144 or more permutations, per successor, are expensive. Under `--compat=tlc` the minimum must be TLC's, so S4's speed-up applies to the default profile only.

## Shipping amendment, 2026-10-09

Questions 14–17 are answered in `docs/open-questions.md`. Implement S3 with `unreduced` as the default, preserving `refuse` and `tlc`. Extend L7 to build two distinct scheduling paths: deterministic level boundaries in the default profile and TLC's actual wall-clock trigger in compat mode. Under final checking, keep full golden comparison. Default unreduced runs are tested against oracle runs without symmetry; no blanket verdict exemption is permitted. Reject malformed permutations by default, and keep S5's model-symmetry audit opt-in. Update SM3's default acceptance cases to sound unreduced verdicts rather than refusals.
