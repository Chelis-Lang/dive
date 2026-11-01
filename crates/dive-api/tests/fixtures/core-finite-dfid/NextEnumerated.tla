---- MODULE NextEnumerated ----
EXTENDS Naturals,FiniteSets
VARIABLES x,s
Init == x = 0 /\ s = {0}
Next == x' \in 0..2 /\ s' = {x'}
Inv == Cardinality(s) = 1
====
