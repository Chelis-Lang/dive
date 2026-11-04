---- MODULE PrimeAlias ----
EXTENDS Naturals, TLC
VARIABLE x
XP == x'
Assign(v,n) == v=n
Forward(v,n) == Assign(v,n)
Init == x=0
A == /\ x<0 /\ Forward(XP,1)
B == Forward(XP,x)
Next == A \/ B
====
