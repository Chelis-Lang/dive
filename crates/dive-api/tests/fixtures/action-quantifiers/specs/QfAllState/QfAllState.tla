---- MODULE QfAllState ----
VARIABLE x
Init == x=TRUE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == []<>(\A i \in {TRUE,FALSE} : x=i)
====
