---- MODULE PrimeArg ----
EXTENDS Naturals
VARIABLE x,y
Assign(v,n) == v=n
Init == /\ x=0 /\ y=0
A == /\ x<0 /\ Assign(x',1) /\ Assign(y',1)
B == /\ Assign(x',x) /\ Assign(y',y)
Next == A \/ B
====
