---- MODULE OpRelayFails ----
EXTENDS TLC
VARIABLE x
Init == x=FALSE
Next == UNCHANGED x
P(v) == v
Use(F(_)) == []<>F(x)
Relay(G(_)) == Use(G)
Live == Relay(P)
====
