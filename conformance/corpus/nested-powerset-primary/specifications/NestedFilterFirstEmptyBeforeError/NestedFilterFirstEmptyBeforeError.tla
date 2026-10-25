---- MODULE NestedFilterFirstEmptyBeforeError ----
EXTENDS Naturals, FiniteSets, TLC
VARIABLES E
Inv == E # {}
Init == E \in {A \in SUBSET (SUBSET (1..5)) : \A e \in A : 1 \div Cardinality(e) = 0}
Next == UNCHANGED <<E>>
====
