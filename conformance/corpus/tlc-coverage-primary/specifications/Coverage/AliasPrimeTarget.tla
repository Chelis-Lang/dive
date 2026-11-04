---- MODULE AliasPrimeTarget ----
EXTENDS Naturals, TLC
VARIABLE x
X == x
Assign(v,n) == v'=n
Forward(v,n) == Assign(v,n)
Init == x=0
A == /\ x<0 /\ Forward(X,1)
B == Forward(X,x)
Next == A \/ B
====
