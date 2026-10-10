---- MODULE ModuleExample ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Init == x = 0
Next == x' = IF x < 2 THEN x + 1 ELSE x
Bound == x \in 0..Cardinality({1, 2}) /\ Len(<<x>>) = 1
====
