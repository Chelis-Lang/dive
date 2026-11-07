---- MODULE StartupEdges ----
\* A constant-level RandomElement fixed at start-up, read by a temporal
\* property through a parameterized operator and a set filter.
EXTENDS Naturals, TLC
Nodes == 1..4
Edges == UNION { {{n, m} : m \in RandomElement(SUBSET (Nodes \ {n}))} : n \in Nodes }
Nbrs(n) == {m \in Nodes : {m, n} \in Edges}
VARIABLE reached
Init == reached = {1}
Next == \E n \in reached : \E m \in Nbrs(n) \ reached : reached' = reached \cup {m}
Spec == Init /\ [][Next]_reached /\ WF_reached(Next)
Closed == \A n \in reached : Nbrs(n) \subseteq reached
Settles == <>[]Closed
NeverAll == []<>(reached # Nodes)
====
