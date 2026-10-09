---- MODULE ErWrapFcnRhs ----
\* WP-C4 error-class matrix: a function constructor applied outside its domain on the right-hand side of a primed assignment.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == \/ x # 2 /\ Step
        \/ x = 2 /\ x' = [i \in 0..1 |-> i][x] /\ y' = y
====
