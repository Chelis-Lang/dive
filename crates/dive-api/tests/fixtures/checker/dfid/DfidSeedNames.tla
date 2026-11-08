---- MODULE DfidSeedNames ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x = CHOOSE v \in {ToString(8), ToString(7)} : TRUE
Next == UNCHANGED x
Inv == x = ToString(7)
====
