---- MODULE RecordMemberBefore ----
EXTENDS Integers, FiniteSets, Sequences
G == INSTANCE Graphs
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Inv == LET p == ((x..(x-1)) \X x) c == [edge |-> p] IN /\ ~(x \in c.edge) /\ G!IsStronglyConnected([node |-> {}, edge |-> c.edge])
====
