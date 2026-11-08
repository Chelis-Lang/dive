---- MODULE ErPostFalse ----
\* WP-C4 error-class matrix: a POSTCONDITION that is FALSE after the search.
EXTENDS Integers, TLC
VARIABLES x, y
Init == x = 0 /\ y = 0
Step == \/ x < 3 /\ x' = x + 1 /\ y' = y
        \/ x' = x /\ y' = 1 - y
Next == Step
Post == TLCGet("distinct") = 3
====
