---- MODULE ReceiverAfterFiniteness ----
EXTENDS Integers, FiniteSets, Sequences
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == LET p == ({} \X x) \X {x}
           c == [edge |-> p]
       IN /\ IsFiniteSet(c.edge)
          /\ ~(x \in c.edge)
====
