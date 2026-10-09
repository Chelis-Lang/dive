---- MODULE SpForwardFair ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Fair(a) == WF_x(a)
Spec == Init /\ [][Next]_x /\ Fair(Next)
P == []<>x
====
