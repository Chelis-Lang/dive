---- MODULE QfNested ----
VARIABLE x
Init == x=TRUE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == []<>(\E i \in {TRUE,FALSE} : \E j \in {i} : x=j)
====
