---- MODULE LazyControl ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == IF TRUE THEN TRUE ELSE Cardinality({<<x>>, <<TRUE>>}) = 2
====
