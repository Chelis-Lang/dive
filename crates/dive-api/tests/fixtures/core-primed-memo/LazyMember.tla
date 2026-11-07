---- MODULE LazyMember ----
\* #869 review: membership in a primed filter over SUBSET is decided on
\* the element, never by building the 2^31 subsets to cache Big'.
EXTENDS Naturals, FiniteSets
VARIABLES x, y
Big == {s \in SUBSET (1..(30 + x)) : Cardinality(s) = 2}
Init == x = 0 /\ y = {1, 2}
Next == /\ x < 3
        /\ x' = x + 1
        /\ y' \in {{1, 2}, {1}, {1, 2, 3}}
        /\ \A i \in 1..3 : y' \in Big'
====
