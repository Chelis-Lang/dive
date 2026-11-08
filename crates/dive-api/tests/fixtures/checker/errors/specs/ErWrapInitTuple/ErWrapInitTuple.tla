---- MODULE ErWrapInitTuple ----
\* WP-C4 error-class matrix: a tuple literal indexed out of range in the initial predicate.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x \in 0..3 /\ y = 0 /\ (x # 2 \/ <<1, 2>>[x + 1] = 1)
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
====
