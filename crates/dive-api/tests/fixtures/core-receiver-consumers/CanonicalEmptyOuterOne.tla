---- MODULE CanonicalEmptyOuterOne ----
EXTENDS Naturals, Sequences, FiniteSets
VARIABLE x
Init == x = 0
Next == UNCHANGED x
Inv == Cardinality({(1..x) \X Nat, {}}) = 1
====
