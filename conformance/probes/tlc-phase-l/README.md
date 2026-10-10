# Bounded phase-L receipts (#140)

This is a staged diagnostic contract, not integrated model checking. All receipts
say `graph_insertion=false`. Strict temporal refusals and previous corpus reports
are unchanged; no corpus verdicts gain match credit.

The complete evidence is split by what it measures:

- `slot-selection.json`: 55 forced-key scalar `SetOfStates` controls. Native
  insertion snapshots match every projected field, including growth on duplicate,
  collision probing, mixed incomparable scalar states, first representative and
  repeat iterator order. These provide no real-model fingerprint evidence.
- `evaluation-selection.json`: 35 synthetic callbacks invoked by actual
  `LiveCheck.addInitState/addNextState`, including multiple OOS groups and deciding
  failures. The test independently checks all callback logs and stop positions;
  it does not claim native SCC/disk insertion from the reference's completion field.
- `model-selection.json`: 51 actual FastTool model-bound calls. Native results
  separate 16 completed exact defined-field projections, 11 exact deciding-error
  projections, 12 SANY-invalid controls, nine constructor refusals (effects,
  introspection, state-IF planning), and three tableau-graph-pending rows.
  Initial particle consistency is separately compared on the latter. Fingerprint,
  slot, first representative, action/state results and deciding error fields are
  measured on real values. None is a native integrated liveness verdict.
- `tlc-selection.json`: 20 clean TLC model runs, including state/action constraints
  and a safety stop. Raw complete results are retained as upcoming graph-insertion
  controls; this slice does not claim native constraint/safety timing equality.

Every current row preserves raw stdout/stderr, exit, timeout, selection and source
hashes. Historical directories preserve all original selected rows and compiler/
probe failures, original undefined Head and invalid action forms, and the native
IF/body-first discrepancies before conservative refusal/direct dispatch repair.
No raw output is edited. `scripts/check-phase-l.py` verifies current and historical
streams, sources and the pre-phase-L planning/tableau matrices separately.

Regeneration requires an explicit reason and the verified pinned jar/JRE. Supply
paths to the cache's Java and a JDK 17 compiler; each Java adapter compiles in a
private temporary directory. All TLC metadata directories are private temporary
paths, and the shared cache is never modified.

```sh
python3 -I conformance/probes/tlc-phase-l/capture-slots.py --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_ORACLE_JAR" --reason "reviewed structural change"
python3 -I conformance/probes/tlc-phase-l/capture-evaluation.py --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_ORACLE_JAR" --reason "reviewed bin control change"
python3 -I conformance/probes/tlc-phase-l/capture-model.py --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_ORACLE_JAR" --reason "reviewed bound control change"
python3 -I conformance/probes/tlc-phase-l/capture-tlc.py --java "$DIVE_JAVA" --jar "$DIVE_ORACLE_JAR" --reason "reviewed clean reference run"
python3 -I scripts/check-phase-l.py
cargo test -p dive-liveness --test phase_l --locked
```

`historical-before-state-if-refusal/observation.json` retains the #147 mismatch:
pinned planning raises 2179 on the constant `1 \div 0` branch, while old native
planning retained a state IF atom. The current Session conservatively refuses
reachable state-kind IF syntax, including helper/redirect IR; it never evaluates
branches to guess safety and gains no agreement credit. This does not repair
ordinary planning.

`historical-before-nonstuttering-order/observation.json` retains #149's error:
pinned `LNAction` skips the erroneous body on a subscript stutter, while old
`PredicateAtom` evaluated it first. The new phase-L dispatch tests subscripts
before body. True/false bodies, both stuttering and changing edges, subscript
2179 and body 2184 failures are separately captured. The source evaluator's
body-first `StepAngle` behavior and independent validator remain separate work.

The next #140 increment must implement actual graph/product insertion, including
its additional predicate/error order, and preserve the logical capacity history
with actual parallel commit evidence. SCC acceptance, periodic timing, VIEW,
symmetry and strict TLC lasso/attribution remain pending.

The retained `historical-opaque-construction` negative control invokes a deferred
provider while `Value::tuple` hashes its child, before the phase-L endpoint runs
(#129). The endpoint controls separately arm their callback trap after composite
construction and require a named refusal for direct and nested opaque inputs.
They provide no constructor-time admission claim.


The shipping integration inherits public fallible evaluator admission and the
shared/independent temporal action-order fix (#149). Original source matrices
and all before-fix observations remain unchanged against archived source bytes.
A separate current integrated source union covers this delivery with those
changes; #147 state-conditional conversion precedence is still conservatively
refused, and graph insertion is still pending.
