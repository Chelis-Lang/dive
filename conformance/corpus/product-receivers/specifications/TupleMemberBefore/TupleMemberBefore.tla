---- MODULE TupleMemberBefore ----
EXTENDS Integers, FiniteSets, Sequences
G == INSTANCE Graphs
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Inv == LET p == ((x..(x-1)) \X x) c == <<p>> IN /\ ~(x \in c[1]) /\ G!IsStronglyConnected([node |-> {}, edge |-> c[1]])
====
