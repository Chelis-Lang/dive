---- MODULE StartupAssumeGraph ----
EXTENDS Naturals
CONSTANTS Nodes, Succ
Reach(n) == {m \in Nodes : m \in Succ[n] \/ m = n}
Closed == \A n \in Nodes : n \in Reach(n)
====
