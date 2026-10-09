---- MODULE LvMixedAction ----
\* PROPERTY Prop == [][x' > x]_x /\ []<>(x = 0). The [][A]_v conjunct is an
\* implied action checked on every step during the search; it fails on the
\* step 2 -> 0. The liveness part holds.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = (x + 1) % 3
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Prop == [][x' > x]_x /\ []<>(x = 0)
====
