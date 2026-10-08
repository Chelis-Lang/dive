---- MODULE MapSetOrder ----
\* Probe: the order of x' \in {e : v \in S}. The map's bound is enumerated in
\* normalized order (1, 2, 3), so if TLC keeps the map's elements in that
\* enumeration order, the counterexample ends in 1; if it kept the literal's
\* order, it would end in 3.
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' \in {v + 0 : v \in {3, 1, 2}}
Inv == x \notin {1, 3}
====
