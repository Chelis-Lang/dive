---- MODULE ErInvReplayProp ----
\* WP-C4 error-class matrix: an invariant failing to evaluate on a step whose action property TLC's replay then finds violated.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
Inv == x # 2 \/ 1 \div 0 = 1
Prop == [][x' # 2]_<<x, y>>
====
