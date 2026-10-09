---- MODULE ErPostNonEnum ----
\* WP-C4 error-class matrix: enumerating an infinite set in a POSTCONDITION after a complete search.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == v # 2 \/ \E n \in Nat : n = v
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
Post == Err(2)
====
