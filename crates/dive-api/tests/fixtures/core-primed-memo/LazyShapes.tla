---- MODULE LazyShapes ----
\* #869 review: primed definitions whose consumers use them without
\* building them: a function constructor over Nat applied and its DOMAIN
\* taken, SUBSET of a large set as a SUBSET-forall filter's domain, and
\* a filter enumerated by streaming.
EXTENDS Naturals, FiniteSets
VARIABLES x, y
F == [n \in Nat |-> n + x]
D == SUBSET (1..(14 + x))
Small == {s \in D : \A e \in s : e < 3}
Even == {e \in 1..(20 + x) : e % 2 = 0 /\ e > 16 + x}
Init == x = 0 /\ y = 0
Next == /\ x < 3
        /\ x' = x + 1
        /\ y' \in Even'
        /\ F'[2] = x' + 2
        /\ 5 \in DOMAIN F'
        /\ Cardinality(Small') = 4
Inv == y = 0 \/ y % 2 = 0
====
