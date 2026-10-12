---- MODULE TupleStorage ----
EXTENDS Naturals
VARIABLES n,s
Init == n=0 /\ s=[i \in 1..2 |-> i]
Next == n'=n+s[1] /\ UNCHANGED s
Inv == n<3
====
