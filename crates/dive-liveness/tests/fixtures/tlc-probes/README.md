# TLC probes for liveness, fairness and symmetry

Minimal specs that pin down how the pinned TLC checks temporal properties, handles fairness, and reduces by symmetry. They are the evidence for ADRs 0017 (TLC's liveness semantics), 0018 (dive's liveness design) and 0019 (symmetry), and the first conformance target of M6 (milestones LM0 and SM1 in `docs/design/m6-plan.md`).

- `specs/<Name>/<Name>.tla` and `.cfg` are the probes. Each module's header comment states the question it asks. `Lv*` probes are about liveness, `Sym*` about symmetry.
- `tlc-2026.10.06.014338/` holds the transcripts, captured with the conformance harness exactly as goldens are (`-workers 1 -fp 0 -seed 0 -lncheck final -tool`, no `-dumpTrace` on the deciding run; the JSON trace comes from the cross-checked trace pass):

  ```sh
  dive-conformance dev capture --oracle tlc-2026.10.06.014338 \
      --specs crates/dive-liveness/tests/fixtures/tlc-probes/specs \
      --out crates/dive-liveness/tests/fixtures/tlc-probes <Name>...
  ```

- `lncheck-default/` holds `LvPeriodic` captured with `--lncheck default` (TLC's default, periodic liveness checks).
- `fp-<N>/` holds the five probes whose counterexample depends on the fingerprint polynomial, captured with `--fp N` (see "Determinism" below).

The transcripts are generated, never edited. Each has `.stdout`, `.stderr`, `.exit`, `.command`, the JSON trace and its `.trace.command` where TLC writes one, and SANY's output.

## Results

"Gen / distinct / queue / depth" are TLC's final statistics (`TLC_STATS` 2199, `TLC_SEARCH_DEPTH` 2194). "Trace" is the number of states printed, then the ending: "back to k" (`TLC_BACK_TO_STATE` 2122) or "stuttering" (`TLC_STATE_PRINT3` 2218). "Warnings" lists 2259 (no fairness), 2279 (symmetry with liveness).

### Liveness and fairness

| Probe | Verdict (exit) | Gen / distinct / queue / depth | Trace | Warnings | What it establishes |
|---|---|---|---|---|---|
| `LvWeakFair` | success (0) | 4 / 3 / 0 / 3 | | | Baseline: `[]<>(x = 0)` under `WF_x(Next)` holds; TLC prints "1 branches" (2212) and checks "the complete state space with 3 total distinct states" (2192) |
| `LvNoFair` | temporal (13) | 4 / 3 / 0 / 3 | 3, stuttering | 2259 | Without fairness, `[]<>(x = 0)` is violated by stuttering. TLC shows stuttering at `x = 2` (3 states), not the shorter stuttering at `x = 1`: its choice is not shortest |
| `LvWFIntermittent` | temporal (13) | 6 / 4 / 0 / 4 | 2, back to 1 | | `WF` does not force an action that is enabled only intermittently |
| `LvSFIntermittent` | success (0) | 6 / 4 / 0 / 4 | | | `SF` does |
| `LvStutterEnd` | temporal (13) | 4 / 4 / 0 / 4 | 4, stuttering | | Stuttering is allowed once the fair action is disabled. No 2259 hint, since the spec has fairness |
| `LvLassoMid` | temporal (13) | 5 / 4 / 0 / 4 | 4, back to 2 | | "Back to state" numbers the printed states from 1; the loop need not contain the initial state |
| `LvTwoLoops` | temporal (13) | 9 / 7 / 0 / 5 | 3, back to 2 | | Of two violating loops, TLC reports the one entered second, the shorter one |
| `LvTwoLoopsRev` | temporal (13) | 9 / 7 / 0 / 5 | 5, back to 2 | | With the loops' lengths swapped, TLC still reports the one entered second, now the longer one. Its choice is neither first in exploration order nor shortest |
| `LvBigCycle` | temporal (13) | 202 / 200 / 0 / 200 | 101, back to 1 | | In a 200-state ring with a chord from 100 back to 0, TLC's cycle uses the chord |
| `LvConstraint` | success (0) | 4 / 3 / 0 / 3 | | | **Liveness is checked on the constrained graph.** No state in the model has `x = 5`, yet `<>(x = 5)` is reported to hold: the only loop at the boundary is stuttering, which `WF` forbids because `ENABLED` ignores `CONSTRAINT` |
| `LvConstraintHolds` | success (0) | 4 / 3 / 0 / 3 | | | The same graph with a property that holds in the model |
| `LvActionConstraint` | temporal (13) | 4 / 2 / 0 / 2 | 2, back to 1 | | `ACTION_CONSTRAINT` removes edges, and the loop that remains is reported, although the unconstrained spec satisfies the property |
| `LvEventuallyAlways` | temporal (13) | 5 / 3 / 0 / 3 | 2, back to 1 | | `<>[]P` (no tableau: an AE-state condition) |
| `LvAlwaysEventually` | temporal (13) | 5 / 3 / 0 / 3 | 2, back to 1 | | `[]<>P` (no tableau: an EA condition) |
| `LvLeadsTo` | temporal (13) | 5 / 3 / 0 / 3 | 3, back to 2 | | `~>` (tableau with a promise) |
| `LvLeadsToHolds` | success (0) | 5 / 3 / 0 / 3 | | | `~>` made true by `SF` on the escaping step |
| `LvMixedState` | invariant (12) | 3 / 3 / 0 / 3 | 3 | | **The `[]P` part of a `PROPERTY` is an invariant.** `Invariant line 11, col 9 to line 11, col 17 of module LvMixedState is violated.` (2110), found during the search, before any liveness check |
| `LvMixedAction` | action property (13) | 4 / 3 / 0 / 3 | 4 | | **The `[][A]_v` part is an implied action** (2112), exit 13 |
| `LvMixedLiveOnly` | temporal (13) | 4 / 3 / 0 / 3 | 3, back to 1 | | Only the `<>` conjunct fails; TLC names the configured property, `Prop` |
| `LvDisjunct` | temporal (13) | 4 / 3 / 0 / 3 | 5, back to 3 | | `[]P \/ <>Q` cannot be split, so liveness checks all of it. The printed lasso repeats states (0, 1, 2, 0, 1, back to 3): it is a product path projected onto states |
| `LvTwoProps` | temporal (13) | 4 / 3 / 0 / 3 | 3, back to 1 | | Two properties: "2 branches" (2212), "6 total distinct states" (3 states × 2), and one counterexample naming both: `Temporal properties P1 and P2 were violated.` |
| `LvPerProcessFair` | success (0) | 13 / 9 / 0 / 5 | | | `\A p \in Procs : WF_vars(Step(p))` |
| `LvStarve` | temporal (13) | 9 / 4 / 0 / 3 | 2, back to 1 | | Fairness for one process only lets the other starve |
| `LvView` | temporal (13) | 3 / 2 / 0 / 2 | 2, back to 1 | | Under `VIEW`, the loop closes on the view: state 2 `(x = 1, y = 1)` goes "back to state 1" `(x = 0, y = 0)`, although its real successor is `(x = 0, y = 2)` |
| `LvEvalError` | error (255) | 1 / 1 / 1 / 1 | | | An evaluation error in a liveness predicate on the initial state is reported wrapped in `GENERAL` (1000), with the initial state's statistics. Before upstream `35d40c91a`, TLC reported success here |
| `LvEvalErrorLater` | error (255) | 4 / 3 / 0 / 3 | 3 | | An error in a liveness predicate during the search: `TLC_MODULE_DIVISION_BY_ZERO` (2179) after the nested-expression trace (2103), "the behavior up to this point" ending at the state being expanded. The statistics include that state's successors: the liveness evaluation comes after them |
| `LvTautology` | success (0) | 4 / 3 / 0 / 3 | | | `[]<>TRUE` is **not** reported as a tautology (2253); it is checked and holds |
| `LvDoubleNeg` | success (0) | 4 / 3 / 0 / 3 | | | `[]~~[](x < 3)` holds (upstream `4260e4712` fixed spurious counterexamples here) |
| `LvEnabledProp` | success (0) | 3 / 3 / 0 / 3 | | | `WF_x(Next)` as a property, which mentions `ENABLED` |
| `LvActionProp` | temporal (13) | 4 / 3 / 0 / 3 | 3, stuttering | 2259 | `[]<><<A>>_v` without fairness |
| `LvPeriodic` | temporal (13) | 4,503,005 / 2,253,003 / 0 / 3001 | 3, back to 2 | | Under `-lncheck final`, the violation is reported after the complete search, with its statistics. See `-lncheck` below |

### Symmetry

| Probe | Verdict (exit) | Gen / distinct / queue / depth | Trace | Warnings | What it establishes |
|---|---|---|---|---|---|
| `SymCountsNone` | success (0) | 55 / 27 / 0 / 7 | | | Baseline: three counters 0..2, no symmetry |
| `SymCounts` | success (0) | 21 / 10 / 0 / 7 | | | With `Permutations(Procs)`: 10 distinct = the 10 orbits. Generated counts only the explored representatives' successors |
| `SymAsym` | success (0) | 13 / 7 / 0 / 5 | | | A declared symmetry the spec does not have (`p1` is special). The counts depend on which member of each orbit is explored first |
| `SymAsymLate` | success (0) | 8 / 5 / 0 / 5 | | | The same with `p3` special: different counts. TLC explores the first-found concrete state, so dive must follow TLC's order |
| `SymTwoSets` | success (0) | 19 / 9 / 0 / 5 | | | `Permutations(A) \cup Permutations(B)` reduces by the **group** they generate: 9 = 3 × 3 orbits, which needs the mixed products |
| `SymView` | success (0) | 7 / 4 / 0 / 4 | | | `SYMMETRY` with `VIEW` |
| `SymViolation` | invariant (12) | 5 / 3 / 0 / 3 | 3 | | An invariant violation under symmetry: the trace length is the breadth-first length |
| `SymOrder` | invariant (12) | 5 / 3 / 0 / 3 | 3 | | The printed trace (`{p3}`, then `{p3, p1}`) shows the states TLC prints, in its model-value order |
| `SymLiveWarn` | success (0) | 2 / 1 / 0 / 1 | | 2279 | Survey probe P4: `<>(x = b)` holds, TLC warns and reports success on one state |
| `SymLiveMissed` | success (0) | 3 / 1 / 0 / 1 | | 2279 | **A missed violation.** `[]<>(x = a)` is false (`a, b, c, b, c, …`), but with `SYMMETRY` TLC reports success |
| `SymLiveMissedNone` | temporal (13) | 7 / 3 / 0 / 2 | 3, back to 2 | | The same spec without `SYMMETRY`: TLC finds the violation |
| `SymLiveSymProp` | temporal (13) | 2 / 1 / 0 / 1 | 1, stuttering | 2279 | **A false violation.** Spec and property are symmetric and the property holds; TLC reports a one-state stuttering counterexample that the spec's fairness forbids |

## `-lncheck`

`LvPeriodic` is an N × N grid (N = 1500, 2,253,003 states) with a two-state trap loop reachable at depth 2, violating `[]<>(a = N)`.

| Run | Check | Gen / distinct / queue / depth | Trace |
|---|---|---|---|
| `-lncheck final` (`tlc-2026.10.06.014338/`) | "complete state space with 2253003 total distinct states" | 4,503,005 / 2,253,003 / 0 / 3001 | 3, back to 2 |
| `-lncheck default`, run 1 (`lncheck-default/`) | "current state space with 417880 total distinct states" | 835,760 / 418,795 / 915 / 915 | the same |
| `-lncheck default`, run 2 | "current state space with 166823 …" | 333,646 / 167,402 / 579 / 579 | the same |
| `-lncheck default`, run 3 | "current state space with 161452 …" | 322,904 / 162,021 / 569 / 569 | the same |

Under the default mode TLC checks liveness periodically, triggered by the wall clock (first after about 3 s). The verdict and the counterexample are the same, but the statistics at the violation vary from run to run. Runs 2 and 3 were captured outside the fixture tree, and only run 1 is kept.

## Determinism

**At one worker, TLC's liveness output is deterministic.** The 19 liveness-violation probes (`LvActionConstraint`, `LvActionProp`, `LvAlwaysEventually`, `LvBigCycle` at N = 2000, `LvDisjunct`, `LvEventuallyAlways`, `LvLassoMid`, `LvLeadsTo`, `LvMixedLiveOnly`, `LvNoFair`, `LvStarve`, `LvStutterEnd`, `LvTwoLoops`, `LvTwoLoopsRev`, `LvTwoProps`, `LvView`, `LvWFIntermittent`, `SymLiveSymProp`, `SymLiveMissedNone`) and the two liveness goldens (`MCDistributedReplicatedLog`, `MCRealTimeHourClock`) were each captured 5 times at `-workers 1 -fp 0 -lncheck final`. Ignoring timestamps and timings, all 105 transcripts were identical, five by five: verdict, named properties, statistics, every counterexample state and label, and the ending.

**At four workers it is not.** Each was also captured 3 times at `--workers 4`. Three changed against the one-worker runs:
- `LvDisjunct`: 5 states (back to 3) or 6 states (back to 4), varying between the three runs;
- `LvLeadsTo`: 3 states (back to 2) or 4 states (back to 3), varying between runs;
- `MCDistributedReplicatedLog`: 4 states naming `AllExtending and InSync`, against 6 states naming `InSync`.

**The fingerprint polynomial changes the counterexample.** TLC orders each state's edges by hash bucket of the successor's fingerprint (`SetOfStates`), so the polynomial (`-fp`) decides which loop its search closes first. The same 21 models were captured at `--fp 1, 2, 3, 7, 42`, one worker. Verdicts and statistics never changed. For 6 of the 21 the counterexample did:

| Model | `-fp 0` (golden setting) | Other polynomials |
|---|---|---|
| `LvTwoLoops` | 3, back to 2 | 5, back to 2 at `-fp 1` and `3`; otherwise 3 |
| `LvTwoLoopsRev` | 5, back to 2 | 3, back to 2 at `-fp 1` and `3`; otherwise 5 |
| `LvLeadsTo` | 3, back to 2 | 4, back to 3 at `-fp 1`, `2` and `7`; otherwise 3 |
| `LvDisjunct` | 5, back to 3 | 6, back to 4 at `-fp 2`; otherwise 5 |
| `SymLiveMissedNone` | 3, back to 2 (`a, b, c`) | `a, c, b` at `-fp 1` and `42` |
| `MCDistributedReplicatedLog` | 6, back to 2, `InSync` | five different lassos of 4, 5 and 6 states over the six polynomials; `-fp 3` and `42` name `AllExtending and InSync` |

The probe transcripts are in `fp-<N>/`. The Examples model's were captured outside the tree; LM3 recaptures them through the harness. TLC's default is a random polynomial per run (`TLC.java:230`), so a TLC user does not see a reproducible liveness counterexample unless they pass `-fp`.
