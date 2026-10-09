---- MODULE ErDeadlockNext ----
\* WP-C4 error-class matrix: an error in the only action of a state that would deadlock.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == x < 3 /\ x' = x + 1 /\ y' = 1 \div (2 - x)
====
