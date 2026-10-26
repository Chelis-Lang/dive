---- MODULE Viol ----
EXTENDS Naturals
VARIABLES x, y
Init == x = 0 /\ y \in {0, 1}
Next == \/ x < 5 /\ x' = x + 1 /\ y' = y
        \/ y = 1 /\ x' = x /\ y' = 0
Small == x + y < 5
====
