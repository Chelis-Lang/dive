---- MODULE ErWrapConstrTuple ----
\* WP-C4 error-class matrix: a tuple literal indexed out of range in a state constraint.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
Constr == x # 2 \/ <<1, 2>>[x + 1] = 1
====
