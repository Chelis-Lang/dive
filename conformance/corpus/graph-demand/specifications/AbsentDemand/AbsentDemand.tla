---- MODULE AbsentDemand ----
EXTENDS Integers, FiniteSets, Sequences
G == INSTANCE Graphs
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Inv == ~G!AreConnectedIn(x, TRUE, [node |-> {x+1}, edge |-> {<<x>>, <<TRUE>>}])
====
