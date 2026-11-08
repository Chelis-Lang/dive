---- MODULE InSetOrder ----
\* Probe: in what order does x' \in S enumerate S? Tool.getNextStates
\* (OPCODE_in) calls elements() rather than elements(NORMALIZED), so it could
\* follow the literal's source order (3 first) instead of normalized order
\* (1 first). The first successor ends the counterexample. Observed: 1, because
\* SetEnumValue's enumerator normalizes the set before iterating.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' \in {3, 1, 2}
Inv == x \notin {1, 3}
====
