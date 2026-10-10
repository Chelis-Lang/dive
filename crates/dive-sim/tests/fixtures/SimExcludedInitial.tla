---- MODULE SimExcludedInitial ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = 0
Bound == x > 0
====
