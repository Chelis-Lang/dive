---- MODULE UndirectedSelf ----
EXTENDS Integers, FiniteSets, Sequences
U == INSTANCE UndirectedGraphs
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Inv == U!AreConnectedIn(x, x, [node |-> {x}, edge |-> {<<x>>, <<TRUE>>}])
====
