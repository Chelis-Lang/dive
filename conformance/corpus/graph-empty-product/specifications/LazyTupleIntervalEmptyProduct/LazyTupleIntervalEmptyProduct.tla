---- MODULE LazyTupleIntervalEmptyProduct ----
EXTENDS Integers, FiniteSets, Sequences
G == INSTANCE Graphs
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x + 1
Inv == LET pair == <<((x..(x-1)) \X x), x>>
       IN G!IsStronglyConnected([node |-> {}, edge |-> pair[1]])
====
