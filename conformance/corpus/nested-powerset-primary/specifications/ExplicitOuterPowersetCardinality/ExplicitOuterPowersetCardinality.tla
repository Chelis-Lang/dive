---- MODULE ExplicitOuterPowersetCardinality ----
EXTENDS Naturals, FiniteSets, TLC
VARIABLES E
Init == E = Cardinality(SUBSET (SUBSET (1..5)))
Next == UNCHANGED <<E>>
====
