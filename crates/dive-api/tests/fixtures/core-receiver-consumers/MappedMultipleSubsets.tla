---- MODULE MappedMultipleSubsets ----
EXTENDS Naturals
VARIABLE x
Init == x = 2
Next == UNCHANGED x
Inv == {{i} : i \in 1..x} \in SUBSET SUBSET (1..x)
====
