---- MODULE ErNextUnassigned ----
\* WP-C4 error-class matrix: reading a primed variable before it is assigned, in the next-state relation.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == \/ x # 2 /\ Step
        \/ x = 2 /\ x' = y' /\ y' = 0
====
