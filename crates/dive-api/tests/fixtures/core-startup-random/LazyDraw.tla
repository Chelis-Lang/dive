---- MODULE LazyDraw ----
\* A precomputed but lazy value: membership in the filter evaluates its
\* predicate, and so draws, while checking. It is not fixed at start-up.
EXTENDS Naturals, TLC
S == {n \in Nat : n = RandomElement(0..1)}
VARIABLE x
Init == x = 0
Next == x' = IF 1 \in S THEN 1 ELSE 0
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live == <>[](x = 0)
====
