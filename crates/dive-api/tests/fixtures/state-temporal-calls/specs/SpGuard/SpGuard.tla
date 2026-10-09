---- MODULE SpGuard ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live(a) == []<>a
Select(p) == IF p THEN []p ELSE <>p
P == []Select(x)
====
