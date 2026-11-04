---- MODULE LazyProductHistory ----
EXTENDS Integers, FiniteSets, Sequences
G == INSTANCE Graphs
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Inv == LET s == {x,TRUE} IN /\ G!IsStronglyConnected([node |-> {}, edge |-> ({} \X s)])
                                  /\ Cardinality(s) = 2
====
