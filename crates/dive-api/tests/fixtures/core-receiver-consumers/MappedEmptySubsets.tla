---- MODULE MappedEmptySubsets ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == LET raw == {x, TRUE} IN {{} : i \in 1..1} \in SUBSET SUBSET raw
====
