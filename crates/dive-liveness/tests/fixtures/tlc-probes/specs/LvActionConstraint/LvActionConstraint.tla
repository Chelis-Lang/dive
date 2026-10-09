---- MODULE LvActionConstraint ----
\* x steps 0, 1, 2 cyclically, or from 1 back to 0. ACTION_CONSTRAINT forbids
\* the step 1 -> 2, so inside the model x only loops 0, 1, 0, ... and []<>(x
\* = 2) is violated by that lasso. Liveness is checked on the constrained
\* graph.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == \/ x' = (x + 1) % 3
        \/ x = 1 /\ x' = 0
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == []<>(x = 2)
NoTwo == ~(x = 1 /\ x' = 2)
====
