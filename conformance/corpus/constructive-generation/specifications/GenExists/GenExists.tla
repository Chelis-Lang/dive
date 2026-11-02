---- MODULE GenExists ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x >= 0 /\ (\E n \in 1..3 : x' = n)
Inv == x = 0
====
