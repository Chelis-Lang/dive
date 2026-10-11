---- MODULE OpTemporalHolds ----
EXTENDS TLC
VARIABLE x
Init == x=TRUE
Next == UNCHANGED x
P(v) == []<>v
Use(F(_)) == F(x)
Live == Use(P)
====
