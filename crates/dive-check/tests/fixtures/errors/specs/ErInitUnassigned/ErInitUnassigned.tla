---- MODULE ErInitUnassigned ----
\* WP-C4 error-class matrix: reading a variable before it is assigned, in the initial predicate.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = y /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
====
