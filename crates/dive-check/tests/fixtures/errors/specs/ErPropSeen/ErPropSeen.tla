---- MODULE ErPropSeen ----
\* WP-C4 error-class matrix: an action property failing on a step into a state already seen.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
Prop == [][y' = y \/ x # 1 \/ y = 0 \/ 1 \div 0 = 1]_<<x, y>>
====
