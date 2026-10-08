---- MODULE ErCmGraphMember ----
EXTENDS Integers, Sequences, FiniteSets, TLC, Graphs
ASSUME PrintT(AreConnectedIn("a", 1, [node |-> 1..2, edge |-> {}]))
====
