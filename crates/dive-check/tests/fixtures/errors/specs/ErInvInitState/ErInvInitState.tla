---- MODULE ErInvInitState ----
\* WP-C4 error-class matrix: an invariant failing to evaluate in an initial state.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x \in 0..3 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
Inv == x # 2 \/ 1 \div 0 = 1
====
