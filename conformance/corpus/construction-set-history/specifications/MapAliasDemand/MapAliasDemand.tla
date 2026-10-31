---- MODULE MapAliasDemand ----
EXTENDS Integers, FiniteSets
CONSTANTS a,b
VARIABLE x
Init == x=0
Next == x<2 /\ x'=x+1
Inv == LET S == {<<i,TRUE>>:i \in {b,a,b}} T == S
       IN /\ <<a,TRUE>> \in T /\ Cardinality(S)=2 /\ <<b,TRUE>> \in S
====
