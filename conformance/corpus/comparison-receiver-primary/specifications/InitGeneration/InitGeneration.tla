---- MODULE InitGeneration ----
EXTENDS Integers, FiniteSets
VARIABLE x

Init == x=0 /\ <<x>> # [a |-> x]
Next == UNCHANGED x
Inv == TRUE
====
