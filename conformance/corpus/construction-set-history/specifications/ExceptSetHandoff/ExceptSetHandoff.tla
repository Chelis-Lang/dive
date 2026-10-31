---- MODULE ExceptSetHandoff ----
EXTENDS Integers, FiniteSets
CONSTANT a
VARIABLES f,x
Init == /\ f=[i \in 1..1 |-> {}] /\ x=0
Next == /\ x<2 /\ x'=x+1
        /\ f'=[f EXCEPT ![1]=@ \cup {<<a,x>>}]
        /\ <<a,x>> \in f'[1]
Inv == Cardinality(f[1])=x
====
