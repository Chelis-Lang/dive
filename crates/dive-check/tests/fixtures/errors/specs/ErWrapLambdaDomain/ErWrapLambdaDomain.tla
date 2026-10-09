---- MODULE ErWrapLambdaDomain ----
\* WP-C4 error-class matrix: a function constructor over a state-dependent domain applied outside it in a guard.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step /\ (x # 2 \/ [i \in 0..x |-> i][x + 5] = 1)
====
