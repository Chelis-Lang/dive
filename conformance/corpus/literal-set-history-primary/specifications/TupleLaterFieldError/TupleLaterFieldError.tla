---- MODULE TupleLaterFieldError ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == x = 0 \/ (Cardinality({<<1, TRUE>>, <<1, 3>>}) = 2)
====
