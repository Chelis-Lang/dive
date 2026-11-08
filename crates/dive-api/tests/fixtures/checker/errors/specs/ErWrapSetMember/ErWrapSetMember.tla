---- MODULE ErWrapSetMember ----
\* WP-C4 error-class matrix: membership of a non-comparable value in a set literal in a guard.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step /\ (x # 2 \/ "a" \in {1, 2})
====
