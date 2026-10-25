---- MODULE RequiredInnerComponentOverflow ----
EXTENDS Naturals, FiniteSets, TLC
VARIABLES E
Inv == E # {}
Init == E \in SUBSET (SUBSET (1..31))
Next == UNCHANGED <<E>>
====
