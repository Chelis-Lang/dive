---- MODULE GraphViolation ----
EXTENDS Integers, FiniteSets, UndirectedGraphs
VARIABLE x, g
Init == /\ x = 0
        /\ g = [node |-> 1..3, edge |-> {}]
Next == /\ x < 2
        /\ x' = x + 1
        /\ g' = [g EXCEPT !.edge = @ \cup {{x+1, x+2}}]
Inv == Cardinality(ConnectedComponents(g)) >= 2
====
