---- MODULE RecordClash ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == IF x=0 THEN TRUE ELSE Cardinality({[a |-> x], [a |-> TRUE]}) = 2
====
