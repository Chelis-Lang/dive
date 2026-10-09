---- MODULE ErWrapLambdaBody ----
\* WP-C4 error-class matrix: a CHOOSE failing inside the body of a function constructor applied in a guard.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step /\ (x # 2 \/ [i \in 0..x |-> CHOOSE j \in {1} : j > 5][0] = 1)
====
