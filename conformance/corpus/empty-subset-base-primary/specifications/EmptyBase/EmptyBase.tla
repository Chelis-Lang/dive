---- MODULE EmptyBase ----
EXTENDS Naturals, TLC
VARIABLE x
Init == x \in SUBSET ({1, TRUE} \X {})
Next == UNCHANGED x
Inv == x # {}
====
