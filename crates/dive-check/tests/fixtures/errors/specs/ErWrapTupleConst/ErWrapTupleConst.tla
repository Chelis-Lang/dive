---- MODULE ErWrapTupleConst ----
\* WP-C4 error-class matrix: a constant tuple indexed out of range in a guard.
EXTENDS Integers, TLC
VARIABLES x, y
Tup == <<1, 2>>
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step /\ (x # 2 \/ Tup[x + 1] = 1)
====
