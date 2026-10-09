---- MODULE LtState ----
VARIABLE x
Init == x=FALSE
Live(p) == LET Seen == p IN []<>Seen
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == Live(x)
====
