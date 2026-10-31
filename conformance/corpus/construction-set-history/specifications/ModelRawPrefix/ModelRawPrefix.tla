---- MODULE ModelRawPrefix ----
EXTENDS Integers, FiniteSets
CONSTANT a
VARIABLE x
Init == x=0
Next == x<1 /\ x'=x+1
Inv == a \in {a,1}
====
