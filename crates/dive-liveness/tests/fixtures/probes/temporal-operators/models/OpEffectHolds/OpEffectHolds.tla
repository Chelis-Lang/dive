---- MODULE OpEffectHolds ----
EXTENDS TLC
VARIABLE x
Init == x=TRUE
Next == UNCHANGED x
P(v) == PrintT(v)
Use(F(_)) == []<>F(x)
Live == Use(P)
====
