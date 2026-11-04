---- MODULE MarkerLazyActual ----
EXTENDS Naturals,TLC
VARIABLE x
Assign(v,n) == v=n
Init == x=0
A == /\ x<0 /\ Assign(x',PrintT("unused"))
B == Assign(x',x)
Next == A \/ B
====
