---- MODULE ErNextRhsDivZero ----
\* WP-C4 error-class matrix: division by zero in the right-hand side of a primed assignment.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == v # 2 \/ 1 \div (v - 2) = 0
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == \/ x < 3 /\ x' = (IF Err(x) THEN x + 1 ELSE x + 1) /\ y' = y
        \/ x' = x /\ y' = 1 - y

====
