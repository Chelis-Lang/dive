---- MODULE ConcreteFactorTree ----
EXTENDS Integers, FiniteSets, Sequences
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == LET p == (SUBSET {x,x+1}) \X [{x,x+1} -> {x}] \X [a : {x,x+1}, b : {x}]
       IN /\ IsFiniteSet(p) /\ Cardinality(p) = 8
====
