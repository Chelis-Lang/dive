---- MODULE LazyNonemptyProduct ----
EXTENDS Integers, FiniteSets, Sequences
G == INSTANCE Graphs
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Inv == G!IsStronglyConnected([node |-> {x,x+1}, edge |-> ({x,x+1} \X {x,x+1})])
====
