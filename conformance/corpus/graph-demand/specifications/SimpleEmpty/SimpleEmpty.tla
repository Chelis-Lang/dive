---- MODULE SimpleEmpty ----
EXTENDS Integers, FiniteSets, Sequences
G == INSTANCE Graphs
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Inv == G!SimplePath([node |-> {}, edge |-> {<<x>>, <<TRUE>>}]) = {}
====
