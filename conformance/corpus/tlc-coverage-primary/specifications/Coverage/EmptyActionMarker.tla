---- MODULE EmptyActionMarker ----
EXTENDS Naturals, TLC
VARIABLE x
Assign(v,n) == v=n
Init == /\ x=0 /\ IF TRUE THEN TRUE ELSE Assign(x,0)
A == \E i \in {} : Assign(x',i)
B == x'=x
Next == A \/ B
====
