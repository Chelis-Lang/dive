---- MODULE LazySubset ----
\* #869 review: a primed SUBSET of 41 or more elements as the domain of a
\* SUBSET-forall filter is rewritten as SUBSET of a small filter (#853),
\* never built to cache D'. The reference engine cannot enumerate it.
EXTENDS Naturals, FiniteSets
VARIABLES x
D == SUBSET (1..(40 + x))
Small == {s \in D : \A e \in s : e < 3}
Init == x = 0
Next == x < 3 /\ x' = x + 1 /\ Cardinality(Small') = 4
====
