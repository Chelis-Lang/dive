---- MODULE Bounded ----
EXTENDS Naturals
VARIABLE x
Init == x=0 /\ FALSE
Next == UNCHANGED x
Live == []<>(x=0)
Hook == TRUE
====
