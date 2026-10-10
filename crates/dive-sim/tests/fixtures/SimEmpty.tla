---- MODULE SimEmpty ----
EXTENDS Naturals
VARIABLE x
Init == x = 0 /\ FALSE
Next == x' = 0
====
