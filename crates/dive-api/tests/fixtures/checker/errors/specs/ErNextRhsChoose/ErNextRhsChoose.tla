---- MODULE ErNextRhsChoose ----
\* WP-C4 error-class matrix: CHOOSE with no satisfying element in the right-hand side of a primed assignment.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == v # 2 \/ (CHOOSE i \in {1, 2} : i > 5) = 1
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == \/ x < 3 /\ x' = (IF Err(x) THEN x + 1 ELSE x + 1) /\ y' = y
        \/ x' = x /\ y' = 1 - y

====
