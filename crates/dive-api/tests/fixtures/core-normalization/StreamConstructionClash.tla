---- MODULE StreamConstructionClash ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == Cardinality({<<x,1>>, IF x=0 THEN <<x+1,2>> ELSE <<TRUE,2>>}) = 2
====
