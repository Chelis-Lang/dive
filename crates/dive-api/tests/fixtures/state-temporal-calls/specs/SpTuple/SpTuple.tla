---- MODULE SpTuple ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Live(a) == []<>a
Both(v) == []<>v[1] /\ []<>v[2]
P == Both(<<x,~x>>)
====
