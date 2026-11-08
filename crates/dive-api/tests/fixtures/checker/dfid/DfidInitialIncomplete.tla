---- MODULE DfidInitialIncomplete ----
EXTENDS Naturals
VARIABLES x,y
Init == (x = 1) \/ (x = 0 /\ y = 0)
Next == x' = x /\ y' = y
====
