---- MODULE ErWrapFilter ----
\* WP-C4 error-class matrix: a CHOOSE failing inside the predicate of a set filter tested for membership in a guard.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step /\ (x # 2 \/ 1 \in {j \in 0..x : (CHOOSE k \in {1} : k > 5) = j})
====
