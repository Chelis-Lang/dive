---- MODULE DfidInitialPrecedence ----
EXTENDS Naturals
VARIABLES x,y
Init == (x = 1) \/ (x = 0 /\ y = 1 \div x)
Next == x' = x /\ y' = y
====
