---- MODULE ErInitNonBool ----
\* WP-C4 error-class matrix: a non-boolean where a formula is expected in generating the initial states.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == IF v = 2 THEN 7 ELSE TRUE
Init == x \in 0..3 /\ y = 0 /\ Err(x)
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step

====
