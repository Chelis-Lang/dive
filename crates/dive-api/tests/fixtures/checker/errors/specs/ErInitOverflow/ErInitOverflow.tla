---- MODULE ErInitOverflow ----
\* WP-C4 error-class matrix: 32-bit integer overflow in generating the initial states.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == v # 2 \/ 2147483647 + v > 0
Init == x \in 0..3 /\ y = 0 /\ Err(x)
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step

====
