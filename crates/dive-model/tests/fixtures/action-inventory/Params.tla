---- MODULE Params ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
A(k) == x' = k
Next == \E k \in {1, 2} : A(k)
====
