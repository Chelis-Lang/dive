---- MODULE UncoveredRecordProducer ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == x = 0 \/ (Cardinality({[a |-> 1, b |-> 2], <<1>>}) = 2)
====
