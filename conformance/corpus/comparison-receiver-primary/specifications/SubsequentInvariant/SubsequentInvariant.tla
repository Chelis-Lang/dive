---- MODULE SubsequentInvariant ----
EXTENDS Integers, FiniteSets
VARIABLE x

Init == x=0
Next == x<2 /\ x'=x+1
Inv == x=0 \/ <<x>> # [a |-> x]
====
