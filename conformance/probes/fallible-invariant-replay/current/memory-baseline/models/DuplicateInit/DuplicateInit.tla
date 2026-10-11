---- MODULE DuplicateInit ----
EXTENDS Integers
VARIABLE x
Init == (x = 0) \/ (x = 0) \/ (x = 1)
Next == /\ x < 3
        /\ x' = x + 1
Inv == x \in 0..3
====
