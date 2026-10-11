---- MODULE EarlyInvariant ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == (x = 0 /\ x' = 1) \/ (x = 0 /\ x' = 2)
Inv == x = 0
====
