---- MODULE ErInitChoose ----
\* WP-C4 error-class matrix: CHOOSE with no satisfying element in generating the initial states.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == v # 2 \/ (CHOOSE i \in {1, 2} : i > 5) = 1
Init == x \in 0..3 /\ y = 0 /\ Err(x)
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step

====
