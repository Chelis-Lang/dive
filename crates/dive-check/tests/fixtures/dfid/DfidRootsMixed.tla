---- MODULE DfidRootsMixed ----
EXTENDS Naturals
VARIABLE x
Init == x \in {0,1}
Next == IF x = 0 THEN x' = 0 ELSE x' = 2
Inv == x < 2
====
