---- MODULE GraphsStep ----
EXTENDS Integers, FiniteSets
G == INSTANCE Graphs
U == INSTANCE UndirectedGraphs
VARIABLE x
Init == x = 0
Next == /\ x < 2
        /\ x' = x + 1
Directed == [node |-> 1..3,
             edge |-> IF x = 0 THEN {} ELSE IF x = 1
                      THEN {<<1,2>>, <<2,3>>}
                      ELSE {<<1,2>>, <<2,3>>, <<3,1>>}]
Undirected == [node |-> 1..4,
               edge |-> IF x = 0 THEN {} ELSE IF x = 1
                        THEN {{1,2}, {2,3}}
                        ELSE {{1,2}, {2,3}, {3,4}}]
Inv == /\ G!AreConnectedIn(1, 3, Directed) = (x > 0)
       /\ G!AreConnectedIn(3, 1, Directed) = (x = 2)
       /\ G!IsStronglyConnected(Directed) = (x = 2)
       /\ Cardinality(G!SimplePath(Directed)) = (IF x = 0 THEN 3 ELSE IF x = 1 THEN 6 ELSE 9)
       /\ U!AreConnectedIn(1, 4, Undirected) = (x = 2)
       /\ Cardinality(U!ConnectedComponents(Undirected)) = (IF x = 0 THEN 4 ELSE IF x = 1 THEN 2 ELSE 1)
       /\ Cardinality(U!SimplePath(Undirected)) = (IF x = 0 THEN 4 ELSE IF x = 1 THEN 10 ELSE 16)
====
