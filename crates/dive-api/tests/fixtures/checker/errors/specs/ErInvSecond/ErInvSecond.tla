---- MODULE ErInvSecond ----
\* WP-C4 error-class matrix: the second of two invariants failing to evaluate.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
InvA == x < 5
InvB == x # 2 \/ 1 \div 0 = 1
====
