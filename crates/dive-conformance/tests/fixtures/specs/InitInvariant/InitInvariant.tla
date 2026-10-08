---- MODULE InitInvariant ----
EXTENDS Naturals
VARIABLE x
Init == x \in {1, 5}
Next == UNCHANGED x
Small == x < 3
====
