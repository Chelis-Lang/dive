---- MODULE HomogeneousControl ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == Cardinality({<<x,1>>, <<x+1,2>>}) = 2 /\ Cardinality({[a |-> x], [a |-> x+1]}) = 2
====
