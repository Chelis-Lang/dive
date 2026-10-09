---- MODULE RndWide ----
EXTENDS Naturals, TLC
VARIABLES x, n
Init == x = 0 /\ n = 0
Next == \E k \in 0..5000 : n' = k /\ x' = RandomElement(1..1000000000)
Inv == n < 5000
====
