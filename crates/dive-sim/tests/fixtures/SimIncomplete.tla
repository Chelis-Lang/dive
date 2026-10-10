---- MODULE SimIncomplete ----
EXTENDS Naturals
VARIABLE x
VARIABLE y
Init == x = 0 /\ y = 0
Next == x' = 1
====
