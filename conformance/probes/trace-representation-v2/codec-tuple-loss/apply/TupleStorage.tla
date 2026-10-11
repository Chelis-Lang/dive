---- MODULE TupleStorage ----
EXTENDS Naturals
VARIABLES n,s
Init == n=0 /\ s=[i \in 1..1 |-> 1]
Next == n'=s["a"] /\ UNCHANGED s
Inv == n<3
====
