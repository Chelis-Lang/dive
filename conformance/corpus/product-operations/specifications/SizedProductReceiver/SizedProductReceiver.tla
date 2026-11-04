---- MODULE SizedProductReceiver ----
EXTENDS Integers, FiniteSets, Sequences
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == LET p == (x..(x+1)) \X (x..(x+1)) c == [edge |-> p]
       IN /\ Cardinality(c.edge) = 4 /\ IsFiniteSet(c.edge) /\ ~(x \in c.edge)
====
