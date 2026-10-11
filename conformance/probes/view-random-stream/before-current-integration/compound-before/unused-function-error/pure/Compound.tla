---- MODULE Compound ----
EXTENDS Naturals,TLC
VARIABLES n,x,s
Init == n=0 /\ x=0 /\ s=[i \in 1..2 |-> 1 \div (i-2)]
Next == n<3 /\ n'=n+1 /\ x'=n+1 /\ UNCHANGED s
Inv == n<3
Projection == n
====
