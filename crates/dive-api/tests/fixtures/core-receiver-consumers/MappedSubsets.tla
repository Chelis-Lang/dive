---- MODULE MappedSubsets ----
EXTENDS Naturals
VARIABLE x
Init == x = 2
Next == UNCHANGED x
Inv == {{i} : i \in 1..1} \in SUBSET SUBSET (1..x)
====
