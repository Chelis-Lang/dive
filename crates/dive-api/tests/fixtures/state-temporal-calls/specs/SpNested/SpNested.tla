---- MODULE SpNested ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live(a) == []<>a
Twice(p) == Live(p) /\ Live(~p)
P == Twice(x)
====
