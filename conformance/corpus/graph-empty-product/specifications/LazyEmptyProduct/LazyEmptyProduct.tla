---- MODULE LazyEmptyProduct ----
EXTENDS Integers, FiniteSets, Sequences
G == INSTANCE Graphs
U == INSTANCE UndirectedGraphs
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Inv == G!IsStronglyConnected([node |-> {}, edge |-> ({} \X {x,TRUE})])
====
