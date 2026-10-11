---- MODULE OpRelayHolds ----
EXTENDS TLC
VARIABLE x
Init == x=TRUE
Next == UNCHANGED x
P(v) == v
Use(F(_)) == []<>F(x)
Relay(G(_)) == Use(G)
Live == Relay(P)
====
