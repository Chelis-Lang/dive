---- MODULE NestedOuterFirstEmpty ----
EXTENDS Naturals, FiniteSets, TLC
VARIABLES E
Inv == E # {}
Init == E \in SUBSET (SUBSET (1..5))
Next == UNCHANGED <<E>>
====
