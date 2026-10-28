---- MODULE TupleReverseClash ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == IF x=0 THEN TRUE ELSE Cardinality({<<TRUE,2>>, <<x,1>>}) = 2
====
