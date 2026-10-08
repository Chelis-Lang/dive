---- MODULE CmGraphs ----
EXTENDS Integers, Sequences, FiniteSets, TLC, Graphs
ASSUME PrintT(AreConnectedIn(1, 3, [node |-> 1..3, edge |-> {<<1, 2>>, <<2, 3>>}]))
ASSUME PrintT(AreConnectedIn(3, 1, [node |-> 1..3, edge |-> {<<1, 2>>, <<2, 3>>}]))
ASSUME PrintT(AreConnectedIn(1, 1, [node |-> 1..3, edge |-> {}]))
ASSUME PrintT(AreConnectedIn(4, 4, [node |-> 1..3, edge |-> {}]))
ASSUME PrintT(AreConnectedIn(1, "x", [node |-> {}, edge |-> {}]))
ASSUME PrintT(AreConnectedIn(1, 3, [node |-> 1..3, edge |-> {<<1, 2>>, <<2, 4>>, <<4, 3>>, <<1>>}]))
ASSUME PrintT(SimplePath([node |-> 1..3, edge |-> {<<1, 2>>, <<2, 3>>, <<3, 1>>}]))
ASSUME PrintT(SimplePath([node |-> {}, edge |-> {}]))
ASSUME PrintT(IsStronglyConnected([node |-> 1..3, edge |-> {<<1, 2>>, <<2, 3>>, <<3, 1>>}]))
ASSUME PrintT(IsStronglyConnected([node |-> 1..3, edge |-> {<<1, 2>>, <<2, 3>>}]))
ASSUME PrintT(IsStronglyConnected([node |-> {}, edge |-> {}]))
====
