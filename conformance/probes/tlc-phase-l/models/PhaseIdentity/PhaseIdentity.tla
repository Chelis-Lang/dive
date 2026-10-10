---- MODULE PhaseIdentity ----
EXTENDS Integers, TLC
VARIABLE x
Init == x = 0
Next == x' \in {0,1,2}
Identity(P) == P
Live == Identity([]<>(x = 2))
====
