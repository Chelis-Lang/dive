---- MODULE InitialFilter ----
EXTENDS Naturals, TLC
VARIABLE x
Init == /\ x \in {0,1} /\ x=0
Next == x'=x
====
