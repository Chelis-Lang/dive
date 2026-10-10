---- MODULE DfidRoots ----
EXTENDS Naturals
VARIABLES x
Init == x \in {0, 1}
Next == x' = x + 2
Inv == x < 2
====
