---- MODULE EnabStep ----
\* ENABLED inside step predicates (04-plan-compiler.md section 5.5): the
\* primed definition NX inside ENABLED reads the ENABLED action's own
\* successor, never the step's. A reused step value would make both fail.
EXTENDS Naturals
VARIABLE x
NX == x'
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x
Prop == [][NX < 3 /\ ENABLED (x' = 7 /\ NX = 7)]_x
AC == NX < 3 /\ ENABLED (x' = 7 /\ NX = 7)
====
