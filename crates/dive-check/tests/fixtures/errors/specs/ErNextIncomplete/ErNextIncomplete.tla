---- MODULE ErNextIncomplete ----
\* WP-C4 error-class matrix: a successor that leaves a variable unassigned.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == \/ x # 2 /\ Step
        \/ x = 2 /\ x' = 3
====
