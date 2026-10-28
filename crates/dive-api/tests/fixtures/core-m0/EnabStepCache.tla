---- MODULE EnabStepCache ----
\* The step's primed-definition thunk must not enter ENABLED's own search.
EXTENDS Naturals
VARIABLE x
NX == x' + 1
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x
Prop == [][NX < 4 /\ ENABLED (x' = 7 /\ NX = 8)]_x
AC == NX < 4 /\ ENABLED (x' = 7 /\ NX = 8)
====
