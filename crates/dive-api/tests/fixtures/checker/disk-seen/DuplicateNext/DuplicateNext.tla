---- MODULE DuplicateNext ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == (x < 3 /\ x' = x + 1) \/ (x < 3 /\ x' = x + 1)
Inv == x \in 0..3
====
