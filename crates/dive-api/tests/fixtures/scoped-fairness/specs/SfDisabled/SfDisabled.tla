---- MODULE SfDisabled ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Go(i) == x#i /\ x'=i
Next == Go(1) \/ UNCHANGED x
Spec == Init /\ [][Next]_x /\ WF_x(Go(1))
P == <>[](x=0)
====
