---- MODULE OrdinaryProductFunctionMember ----
EXTENDS Integers, FiniteSets, Sequences, TLC
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == [i \in 1..2 |-> x] \in ((x..(x+1)) \X (x..(x+1)))
====
