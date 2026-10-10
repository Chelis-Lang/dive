# Temporal action evaluation order (#149)

The pinned `LNAction.eval` compares the subscript in the two states first. A
stutter returns FALSE for `<<A>>_v` and TRUE for `[A]_v` without evaluating A.
Ordinary `Tool.eval` evaluates the action body first. These are separate
reference contracts, even though their boolean results agree on error-free inputs.

The retained `ActionOrder.tla` has:

```tla
Body == IF x' = 1 THEN 1 \div 0 = 0 ELSE x' = 2
Runtime == <<Body>>_x
```

On x=1 to x=1, the actual temporal angle returns FALSE, the box returns TRUE,
and ordinary runtime evaluation raises 2179. A non-stutter to x=1 still raises
2179. `FalseBody` paired with `BadSub` proves a FALSE body must not skip a
subscript error. `capture.json` retains every one of 36 direct calls, raw bytes,
source hashes and the pinned jar digest. These are primitive evaluation controls,
not whole model-checking verdicts or integrated liveness certification.

`PredicateAtom::evaluate` now follows that temporal order, after non-evaluating
model/ID/scope admission. The independent periodic evaluator implements the
same contract separately for direct angle/box atoms and fairness occurrences,
including the actual closing edge. It still decides ENABLED independently.
The runtime evaluator and its body-first behavior are unchanged. Purity,
finite binding and scope restrictions still yield named refusals; broader
planning error precedence and strict graph/SCC/witness integration remain pending.

The native tests cover all 27 angle/bad-sub/runtime controls and independently
cover all 18 angle/box controls, along with recurring fairness, closing stutters,
non-stutter errors and invalid IDs. Before-fix native failures and their original
source are retained under `native-before`; the first Java adapter compilation
failure is retained separately with zero executed rows and no oracle credit.
Prior planning/context/tableau source matrices remain exact against archived
source bytes. No generated golden is edited.

Reproduce with the pinned jar and JDK 17 compiler:

```sh
python3 -I conformance/probes/temporal-action-order/capture.py \
  --java "$DIVE_JAVA" --javac "$DIVE_JAVAC" --jar "$DIVE_JAR" \
  --reason 'Explicit reason for updating all direct controls'
python3 -I scripts/check-temporal-action-order.py
cargo test -p dive-liveness --test planning -p dive-trace --test temporal --locked
```
