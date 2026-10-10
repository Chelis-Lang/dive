---- MODULE DfidDeadlock ----
EXTENDS Naturals
VARIABLES x
Init == x = 0
Next == x > 0 /\ x' = x
====
