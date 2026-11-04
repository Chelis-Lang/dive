---- MODULE QuantSet ----
EXTENDS Naturals, TLC
VARIABLE x
All(S) == \A i \in S: i>=1
Init == /\ x=0 /\ All({1,2,3})
Next == x'=x
====
