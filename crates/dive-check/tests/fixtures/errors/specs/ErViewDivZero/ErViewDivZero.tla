---- MODULE ErViewDivZero ----
\* WP-C4 error-class matrix: division by zero in VIEW.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == v # 2 \/ 1 \div (v - 2) = 0
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
View == IF Err(x) THEN <<x, y>> ELSE <<y, x>>
====
