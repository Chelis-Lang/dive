---- MODULE SimInitialInvariant ----
EXTENDS Naturals
VARIABLE x
Init == x \in {0, 1}
Next == x' = x
Inv == x = 0
====
