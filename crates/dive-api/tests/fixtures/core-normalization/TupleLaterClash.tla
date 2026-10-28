---- MODULE TupleLaterClash ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == IF x=0 THEN TRUE ELSE Cardinality({<<x,1>>, <<x,TRUE>>}) = 2
====
