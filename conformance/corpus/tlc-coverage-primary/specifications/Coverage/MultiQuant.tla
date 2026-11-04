---- MODULE MultiQuant ----
EXTENDS Naturals
VARIABLE x
Both(S) == \A i,j \in S: i+j>=2
Init == /\ x=0 /\ Both({1,2})
Next == x'=x
====
