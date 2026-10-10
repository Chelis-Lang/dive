---- MODULE SimInvariant ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' \in {1, 2}
Inv == x < 2
====
