---- MODULE NestedSetClash ----
EXTENDS Integers, FiniteSets
VARIABLE x
Init == x = 0
Next == x < 2 /\ x' = x + 1
Inv == IF x=0 THEN TRUE ELSE Cardinality({{x}, {TRUE}}) = 2
====
