---- MODULE RndCoincidence ----
EXTENDS Naturals, TLC
VARIABLES x, n
Init == x = FALSE /\ n = 0
Next == \E k \in 0..1 : n' = k /\ x' = RandomElement({TRUE})
Inv == n < 1
====
