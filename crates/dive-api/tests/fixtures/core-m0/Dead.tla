---- MODULE Dead ----
EXTENDS Naturals
VARIABLE x
Init == x \in 0..2
Next == x < 4 /\ x' = x + 2
====
