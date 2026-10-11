---- MODULE OpConstantFails ----
EXTENDS TLC
VARIABLE x
Init == x=FALSE
Next == UNCHANGED x
K(v) == TRUE
Use(F(_)) == []F(x)
Live == Use(K)
====
