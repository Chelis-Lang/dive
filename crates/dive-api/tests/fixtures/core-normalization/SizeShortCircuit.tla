---- MODULE SizeShortCircuit ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == Cardinality({<<x>>, <<TRUE,2>>}) = 2
====
