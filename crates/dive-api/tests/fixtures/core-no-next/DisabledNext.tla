---- MODULE DisabledNext ----
EXTENDS Naturals
VARIABLE x
Init == x = 0
Next == x' = x /\ x < 0
====
