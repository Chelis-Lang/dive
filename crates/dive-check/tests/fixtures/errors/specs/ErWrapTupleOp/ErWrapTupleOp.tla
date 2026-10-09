---- MODULE ErWrapTupleOp ----
\* WP-C4 error-class matrix: a tuple literal indexed out of range inside an operator in a guard.
EXTENDS Integers, TLC
VARIABLES x, y
T(v) == <<1, 2>>[v + 1]
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step /\ (x # 2 \/ T(x) = 1)
====
