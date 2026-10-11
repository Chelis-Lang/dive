---- MODULE OpTemporalFails ----
EXTENDS TLC
VARIABLE x
Init == x=FALSE
Next == UNCHANGED x
P(v) == []<>v
Use(F(_)) == F(x)
Live == Use(P)
====
