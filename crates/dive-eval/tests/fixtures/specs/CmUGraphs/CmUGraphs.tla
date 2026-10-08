---- MODULE CmUGraphs ----
EXTENDS Integers, Sequences, FiniteSets, TLC, UndirectedGraphs
ASSUME PrintT(AreConnectedIn(3, 1, [node |-> 1..4, edge |-> {{1, 2}, {2, 3}}]))
ASSUME PrintT(AreConnectedIn(4, 1, [node |-> 1..4, edge |-> {{1, 2}, {2, 3}}]))
ASSUME PrintT(ConnectedComponents([node |-> 1..5, edge |-> {{1, 2}, {3, 4}, {4}, {5, 6}}]))
ASSUME PrintT(ConnectedComponents([node |-> {}, edge |-> {}]))
ASSUME PrintT(SimplePath([node |-> 1..3, edge |-> {{1, 2}, {2, 3}}]))
====
