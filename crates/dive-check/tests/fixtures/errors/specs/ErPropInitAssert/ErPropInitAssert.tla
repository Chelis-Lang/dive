---- MODULE ErPropInitAssert ----
\* WP-C4 error-class matrix: a failed Assert in a state-predicate PROPERTY (an implied initial predicate).
EXTENDS Integers, TLC
VARIABLES x, y
Err(v) == Assert(v # 2, "boom")
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
Prop == Err(x + 2)
====
