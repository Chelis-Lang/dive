---- MODULE LtDisabled ----
VARIABLE x
Init == x=FALSE
Go == LET Can == ~x Step == Can /\ x'=TRUE IN Step
Next == Go \/ (x /\ x'=x)
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == []<>(~x)
====
