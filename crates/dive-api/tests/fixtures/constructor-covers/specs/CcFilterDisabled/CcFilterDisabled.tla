---- MODULE CcFilterDisabled ----
VARIABLE x
Init == x=0
Go == x#1 /\ x'=1 /\ {b \in {0,1} : b=x'}={1}
Next == Go \/ UNCHANGED x
Spec == Init /\ [][Next]_x /\ WF_x(Go)
P == <>[](x=0)
====
