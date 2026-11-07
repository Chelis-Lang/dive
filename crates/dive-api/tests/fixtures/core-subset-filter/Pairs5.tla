---- MODULE Pairs5 ----
\* SUBSET (SUBSET (1..5)) has 2^32 elements; the filter is SUBSET of its 10 pairs.
EXTENDS FiniteSets, Naturals
VARIABLE Edges
Pairs == {E \in SUBSET (SUBSET (1..5)) : \A e \in E : Cardinality(e) = 2}
Init == Edges \in Pairs
Next == UNCHANGED Edges
TypeOK == Edges \in Pairs
====
