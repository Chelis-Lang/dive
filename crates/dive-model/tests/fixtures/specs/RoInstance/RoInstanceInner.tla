---- MODULE RoInstanceInner ----
EXTENDS Naturals
VARIABLE c
Init == c = 0
Next == c' = 1 - c
Spec == Init /\ [][Next]_c
====
