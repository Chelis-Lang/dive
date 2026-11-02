---- MODULE GenDisj ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Step == \/ x' = 1
        \/ x' = 2
Next == x >= 0 /\ Step
Inv == x = 0
====
