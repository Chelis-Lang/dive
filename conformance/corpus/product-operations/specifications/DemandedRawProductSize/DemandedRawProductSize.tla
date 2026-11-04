---- MODULE DemandedRawProductSize ----
EXTENDS Integers, FiniteSets, Sequences
VARIABLE x
Init == x = 0
Next == /\ x < 2 /\ x' = x+1
Inv == LET s == {x,TRUE} p == s \X {x}
       IN Cardinality(p) > 0
====
