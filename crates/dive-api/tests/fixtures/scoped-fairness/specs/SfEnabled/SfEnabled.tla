---- MODULE SfEnabled ----
EXTENDS Naturals
VARIABLE x
Init == x=0
Go(i) == x#i /\ x'=i
Next == Go(0) \/ Go(1)
Spec == Init /\ [][Next]_x /\ WF_x(Go(0)) /\ WF_x(Go(1))
P == []<>(ENABLED Go(1))
====
