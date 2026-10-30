---- MODULE TuplePermutationReverse ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == x = 0 \/ (Cardinality({<<3, FALSE>>, <<2, 1>>, <<1, TRUE>>}) = 3)
====
