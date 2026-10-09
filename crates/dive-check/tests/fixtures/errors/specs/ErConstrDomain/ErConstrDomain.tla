---- MODULE ErConstrDomain ----
\* WP-C4 error-class matrix: a function applied outside its domain in a state constraint.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == v # 2 \/ [i \in 0..1 |-> i][v] = 0
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
Constr == Err(x)
====
