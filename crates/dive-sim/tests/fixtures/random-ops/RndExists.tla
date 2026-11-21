---- MODULE RndExists ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == \E i \in 1..2 : x' = 10 * RandomElement(1..1000) + i
Safe == x >= 0
====
