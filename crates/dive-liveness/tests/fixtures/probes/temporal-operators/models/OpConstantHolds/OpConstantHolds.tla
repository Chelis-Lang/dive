---- MODULE OpConstantHolds ----
EXTENDS TLC
VARIABLE x
Init == x=TRUE
Next == UNCHANGED x
K(v) == TRUE
Use(F(_)) == []F(x)
Live == Use(K)
====
