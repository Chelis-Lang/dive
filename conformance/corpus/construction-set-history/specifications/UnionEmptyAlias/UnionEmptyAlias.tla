---- MODULE UnionEmptyAlias ----
EXTENDS Integers, FiniteSets
CONSTANT a
VARIABLE x
Init == x=0
Next == x<1 /\ x'=x+1
Inv == LET S == {<<a,TRUE>>} T == {} \cup S
       IN /\ <<a,TRUE>> \in T /\ Cardinality(T)=1 /\ <<a,TRUE>> \in S
====
