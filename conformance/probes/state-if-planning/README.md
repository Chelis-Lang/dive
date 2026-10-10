# Conditional planning error order

The pinned TLC jar is `tlc-2026.10.06.014338`, revision
`94d0c5070852b37b801bb8998c436383ab7ee37b`. `PlanningProbe.java` invokes its actual
`Liveness.processLiveness` before any graph or state fingerprint exists.

```tla
VARIABLE x
Prop == []<>(IF x = 1 THEN 1 \div 0 = 0 ELSE x = 2)
```

TLC raises code **2179** during planning. It also evaluates an erroneous constant
ELSE branch when the guard is state-dependent. An empty-state guard that succeeds
selects only one branch. A user-defined call can swallow this conversion error;
state-level implication converts its operands, while equivalence stays atomic.

The complete 24-row capture includes 22 planning cases and two SANY rejections
before planning. `historical-original-18` preserves the complete prior selection.
The initial adapter stopped on an invalid action wrapper; its source, outputs
and log remain in `historical-action-wrapper-rejection`. `native-before` retains
the actual false completion, source bytes and development failures. All rows
remain visible; these are planning controls, not corpus verdict certification.

Run `capture.py --java JAVA --javac JAVAC --jar JAR --reason REASON` with the
pinned JDK/jar to reproduce. Preserve an existing capture before regeneration;
the script refuses to overwrite it. `scripts/check-state-if-planning.py` verifies
all raw/source bytes and the selections. ADR 0036 records the native counterpart.
