---- MODULE ConstrainedViolation ----
\* Probe: is a successor that fails CONSTRAINT still checked against the
\* invariants? Worker.addElement treats a not-in-model successor as unseen, so
\* TLC should report the violation at x = 2 even though 2 is outside the model.
\* It should not count 2 as a distinct state, and the reported depth should
\* come only from states inside the model.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x + 1
Inv == x # 2
Constr == x < 2
====
