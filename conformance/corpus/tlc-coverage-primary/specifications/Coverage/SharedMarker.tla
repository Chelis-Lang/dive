---- MODULE SharedMarker ----
EXTENDS Naturals, TLC
VARIABLE x
Assign(v,n) == v=n
Init == /\ x=0 /\ IF TRUE THEN TRUE ELSE Assign(x,0)
A == /\ x<0 /\ Assign(x',1)
B == Assign(x',x)
Next == A \/ B
====
