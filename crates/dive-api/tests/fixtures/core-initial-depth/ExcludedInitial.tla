---- MODULE ExcludedInitial ----
EXTENDS Naturals
VARIABLE x
Init == x \in 0..2
Next == UNCHANGED x
Bound == x > 2
====
