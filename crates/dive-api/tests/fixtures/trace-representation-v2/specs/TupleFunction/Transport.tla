---- MODULE Transport ----
EXTENDS Naturals
VARIABLES n,s
Init == n=0 /\ s=[i \in 1..2 |-> i]
Next == n<3 /\ n'=n+1 /\ UNCHANGED s
Inv == n<3
====
