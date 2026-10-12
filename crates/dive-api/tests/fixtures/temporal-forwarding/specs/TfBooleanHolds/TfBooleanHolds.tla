---- MODULE TfBooleanHolds ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Identity(p) == p
Combine(p,q) == p /\ q
P == Combine([]<>(~x), []<>x)
====
