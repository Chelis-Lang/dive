---- MODULE ErInvReplayViolation ----
\* WP-C4 error-class matrix: an invariant failing to evaluate, after which TLC's replay meets a violation of a second invariant.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
InvA == x # 2 \/ 1 \div 0 = 1
InvB == y = 0
====
