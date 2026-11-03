---- MODULE DemandedEndpoint ----
EXTENDS Integers, FiniteSets, Sequences
G == INSTANCE Graphs
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Inv == G!AreConnectedIn(x, x+1, [node |-> {x, x+1}, edge |-> {<<TRUE, x+1>>}])
====
