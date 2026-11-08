---- MODULE ErActConstrAssert ----
\* WP-C4 error-class matrix: a failed Assert in an action constraint.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == Assert(v # 2, "boom")
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
ActConstr == Err(x)
====
