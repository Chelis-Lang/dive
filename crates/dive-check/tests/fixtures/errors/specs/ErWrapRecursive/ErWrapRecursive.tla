---- MODULE ErWrapRecursive ----
\* WP-C4 error-class matrix: a CHOOSE failing inside a recursive function in a guard.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
f[n \in Nat] == IF n = 0 THEN CHOOSE k \in {1} : k > 5 ELSE f[n - 1]
Next == Step /\ (x # 2 \/ f[x] = 1)
====
