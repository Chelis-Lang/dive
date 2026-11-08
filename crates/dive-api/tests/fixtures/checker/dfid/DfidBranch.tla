---- MODULE DfidBranch ----
EXTENDS Naturals
VARIABLES x
Init == x = 0
Next == IF x = 0 THEN x' \in {1, 2} ELSE x' = x + 2
Inv == x < 3
====
