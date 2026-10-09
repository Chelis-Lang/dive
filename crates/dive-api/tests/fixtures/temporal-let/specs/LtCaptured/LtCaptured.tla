---- MODULE LtCaptured ----
VARIABLE x
Init == x=FALSE

Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == \A i \in {TRUE,FALSE} : LET Match == x=i IN []<>Match
====
