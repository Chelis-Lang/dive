---- MODULE SpHold ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live(a) == []<>a
P == Live(x)
====
