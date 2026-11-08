---- MODULE DeterministicError ----
EXTENDS Integers
VARIABLE x
Init == x = 0
Next == (x = 0 /\ x' = 1) \/ (x = 1 /\ x' = [a |-> 2].missing)
====
