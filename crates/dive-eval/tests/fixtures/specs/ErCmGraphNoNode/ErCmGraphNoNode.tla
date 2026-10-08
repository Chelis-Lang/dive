---- MODULE ErCmGraphNoNode ----
EXTENDS Integers, Sequences, FiniteSets, TLC, Graphs
ASSUME PrintT(AreConnectedIn(1, 2, [edge |-> {}]))
====
