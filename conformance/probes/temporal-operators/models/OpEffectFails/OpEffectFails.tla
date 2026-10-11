---- MODULE OpEffectFails ----
EXTENDS TLC
VARIABLE x
Init == x=FALSE
Next == UNCHANGED x
P(v) == PrintT(v)
Use(F(_)) == []<>F(x)
Live == Use(P)
====
