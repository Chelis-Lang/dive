---- MODULE OrdinaryTupleProductCardinality ----
EXTENDS Integers, FiniteSets, Sequences, TLC
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == Cardinality((<<((x..x) \X (x..x))>>)[1]) = 1
====
