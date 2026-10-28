---- MODULE EnabStepSearch ----
\* ENABLED's third candidate is a witness: its primed body is not a step thunk.
EXTENDS Naturals
VARIABLE x
NX == x' + 1
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x
Prop == [][ENABLED (x' \in 0..2 /\ NX = 3)
           /\ ~ENABLED (x' \in 0..2 /\ NX = 4)]_x
AC == ENABLED (x' \in 0..2 /\ NX = 3)
      /\ ~ENABLED (x' \in 0..2 /\ NX = 4)
====
