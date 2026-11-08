---- MODULE ErInitNonBoolIf ----
\* WP-C4 error-class matrix: a non-boolean IF condition in generating the initial states.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == IF (IF v = 2 THEN 7 ELSE TRUE) THEN TRUE ELSE FALSE
Init == x \in 0..3 /\ y = 0 /\ Err(x)
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step

====
