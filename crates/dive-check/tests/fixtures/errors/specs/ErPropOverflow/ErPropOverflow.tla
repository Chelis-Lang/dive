---- MODULE ErPropOverflow ----
\* WP-C4 error-class matrix: 32-bit integer overflow in an action property [][A]_v.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == v # 2 \/ 2147483647 + v > 0
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
Prop == [][Err(x)]_<<x, y>>
====
