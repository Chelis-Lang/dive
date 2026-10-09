---- MODULE ErWrapRecord ----
\* WP-C4 error-class matrix: a missing record field in a guard.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step /\ (x # 2 \/ [a |-> 1]["b"] = 1)
====
