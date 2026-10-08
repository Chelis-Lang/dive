---- MODULE ErCmGraphEdgeNonSet ----
EXTENDS Integers, Sequences, FiniteSets, TLC, UndirectedGraphs
ASSUME PrintT(ConnectedComponents([node |-> {1}, edge |-> 3]))
====
