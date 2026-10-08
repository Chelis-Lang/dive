# TLC probes for breadth-first search semantics

Minimal specs that pin down how the pinned TLC counts, orders and stops its breadth-first search. They are the evidence for ADRs 0014 and 0015 and the first conformance target of `dive-check` (milestone CM0 in `docs/design/m5-plan.md`).

- `specs/<Name>/<Name>.tla` and `.cfg` are the probes. Each module's header comment states the question it asks.
- `tlc-2026.10.06.014338/` holds the transcripts, captured with the conformance harness:

  ```sh
  dive-conformance dev capture --oracle tlc-2026.10.06.014338 \
      --specs crates/dive-check/tests/fixtures/tlc-probes/specs \
      --out crates/dive-check/tests/fixtures/tlc-probes <Name>...
  ```

  `dev capture` runs TLC exactly as golden generation does: `-workers 1 -fp 0 -seed 0 -tool -dumpTrace json`, deadlock checking as the cfg says. Each probe has `.stdout`, `.stderr`, `.exit`, `.command`, the `-dumpTrace` JSON where TLC writes one, and SANY's output. The transcripts are generated, never edited.

## Results

"Gen / distinct / queue / depth" are TLC's final statistics (`TLC_STATS`, `TLC_SEARCH_DEPTH`). "Trace" is the counterexample's number of states and its last state.

| Probe | Verdict (exit) | Gen / distinct / queue / depth | Trace | What it establishes |
|---|---|---|---|---|
| `MidLevelViolation` | invariant (12) | 3 / 3 / 1 / 2 | 2, ends `x = 2` | TLC stops at the violating successor. It counts every successor generated so far, including the violating one, and nothing after it (3 and 10 are never generated). The violating state is in the distinct count but not on the queue |
| `ConstrainedViolation` | invariant (12) | 3 / 2 / 0 / 2 | 3, ends `x = 2` | A successor outside `CONSTRAINT` is still checked against the invariants, and the violation is reported. It is generated but not distinct. The depth (2) is less than the trace length (3), because depth only counts states inside the model |
| `ConstraintNoDeadlock` | success (0) | 3 / 2 / 0 / 2 | | A state whose only successor fails `CONSTRAINT` is not a deadlock: deadlock means no successor was generated at all |
| `ActionConstraintNoDeadlock` | success (0) | 3 / 2 / 0 / 2 | | The same for `ACTION_CONSTRAINT` |
| `InitViolation` | invariant, initial (12) | none printed | 1, `x = 2` | An initial-state violation prints no statistics at all. ("Computed 2 initial states..." is progress output) |
| `StutterNext` | success (0) | 3 / 3 / 0 / 3 | | Baseline for the next two probes |
| `StutterSpec` | deadlock (11) | 3 / 3 / 0 / 3 | 3, ends `x = 2` | With `SPECIFICATION Init /\ [][Next]_x`, TLC explores `Next`, not `[Next]_x`: no stuttering successors, and deadlock is reported |
| `StutterInNext` | success (0) | 6 / 3 / 0 / 3 | | With `NEXT [Next]_x`, each state also generates its stuttering step, which counts as generated and prevents deadlock |
| `DuplicateSuccessors` | success (0) | 11 / 2 / 0 / 2 | | Generated counts every successor the enumeration produces, duplicates included: `x' = 1 \/ x' = 1` gives 2, `x' \in {0, 1} /\ x' = 1` gives 1, and `\E i \in {1, 2} : x' = 1` gives 2 |
| `EvalErrorMidLevel` | evaluation error (255) | 4 / 3 / 1 / 2 | 2, ends `x = 1` (the state being expanded) | An evaluation error part-way through a state's successors keeps the counts of everything generated before it. The trace ends at the state being expanded, and the step is labelled with the split action's name (`A`) |
| `DeadlockMidLevel` | deadlock (11) | 4 / 4 / 1 / 3 | 2, ends `x = 2` | Deadlock is found when the deadlocked state is expanded, after states before it in the queue were expanded. The depth (3) exceeds the trace length (2) |
| `ExistsOrder` | invariant (12) | 3 / 3 / 1 / 2 | 2, ends `x = 21` | `\E i \in S, j \in T` varies the **first** bound variable fastest: 11, then 21 |
| `ExistsSetOrder` | invariant (12) | 2 / 2 / 0 / 2 | 2, ends `x = 1` | `\E` enumerates its bound in normalized order, not the literal's order |
| `InSetOrder` | invariant (12) | 2 / 2 / 0 / 2 | 2, ends `x = 1` | `x' \in {3, 1, 2}` also yields 1 first: an enumerated set is normalized before it is iterated |
| `FilterSetOrder` | invariant (12) | 2 / 2 / 0 / 2 | 2, ends `x = 1` | The same for `x' \in {v \in S : P}` |
| `MapSetOrder` | invariant (12) | 2 / 2 / 0 / 2 | 2, ends `x = 1` | The same for `x' \in {e : v \in S}` |
| `StringOrderViolation` | invariant `InvZeta` (12) | 2 / 2 / 0 / 2 | 2, ends `y = "zeta"` | Set enumeration order decides which violation is reported. In TLC's intern order `"zeta"` comes before `"alpha"`, so TLC reports `InvZeta`. Lexicographic order would report `InvAlpha` |
| `StringOrderPreinterned` | invariant `InvA` (12) | 2 / 2 / 0 / 2 | 2, ends `y = "a"` | TLC's string order is not the order of first appearance in the user's module: `a` and `b` were already interned from standard modules TLC always loads |
| `ImpliedActionSeen` | action property (13) | 3 / 2 / 0 / 2 | 3, ends `x = 0` | Action properties (`[][A]_v`) are checked on every generated step, including steps into states already seen |

**Which modules TLC loads.** The SANY lines of every transcript show TLC parsing `Naturals`, `_JsonTrace`, `TLC`, `TLCExt`, `Json`, `Sequences`, `FiniteSets` and `Integers` even for a module that extends nothing. `_JsonTrace` and its dependencies are there because of `-dumpTrace json`, which golden generation always passes. TLC's intern order, and so its string order under `--choose-order=tlc`, can therefore depend on TLC's command-line flags, not only on the spec. Whether a run without `-dumpTrace json` orders strings differently is not yet measured.

Two further facts come from the existing harness fixtures in `crates/dive-conformance/tests/fixtures/`:
- `EvalError`: an error while expanding a state is reported as `GENERAL` (1000), then replayed and reported again as `TLC_FINGERPRINT_EXCEPTION` (2147). The trace ends at the state being expanded.
- `AssertFailure`: `Assert` failing exits 14.

Survey probe P8 (`docs/survey/competitive.md`) is the larger determinism probe. With one worker, TLC stops after 63,251 states with a 46-state trace. With eight workers, the count varies from run to run.
