---- MODULE InitEnumerated ----
EXTENDS Naturals,FiniteSets
VARIABLES x,s
Init == x \in 0..2 /\ s = {x}
Next == UNCHANGED <<x,s>>
Inv == Cardinality(s) = 1
====
