---- MODULE GenPartial ----
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y = 0
Next == x >= 0 /\ (\E n \in 1..3 : x' = n)
====
