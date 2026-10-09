---- MODULE ErInitIncomplete ----
\* WP-C4 error-class matrix: an initial state that leaves a variable unassigned.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x \in 0..1
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
====
