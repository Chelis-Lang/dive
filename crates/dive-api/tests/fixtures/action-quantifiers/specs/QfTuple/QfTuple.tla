---- MODULE QfTuple ----
VARIABLE x
Init == x=TRUE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == []<>(\A <<i,j>> \in {<<TRUE,FALSE>>,<<FALSE,TRUE>>} : x=i \/ x=j)
====
