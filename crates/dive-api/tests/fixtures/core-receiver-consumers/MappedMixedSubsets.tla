---- MODULE MappedMixedSubsets ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == {{x, TRUE} : i \in 1..1} \in SUBSET SUBSET {}
====
