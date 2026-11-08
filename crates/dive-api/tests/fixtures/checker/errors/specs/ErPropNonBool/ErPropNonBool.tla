---- MODULE ErPropNonBool ----
\* WP-C4 error-class matrix: a non-boolean where a formula is expected in an action property [][A]_v.
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == IF v = 2 THEN 7 ELSE TRUE
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
Prop == [][Err(x)]_<<x, y>>
====
