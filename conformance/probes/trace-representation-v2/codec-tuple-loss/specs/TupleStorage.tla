---- MODULE TupleStorage ----
EXTENDS Naturals
VARIABLES n,s
Init == n=0 /\ s=[i \in 1..1 |-> 1]
Next == n<3 /\ [a |-> 1,b |-> 1] # s /\ n'=n+1 /\ UNCHANGED s
Inv == n<3
====
