---- MODULE LtNested ----
VARIABLE x
Init == x=FALSE
Live(p) == LET Follow(q) == []<>q IN Follow(p)
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == Live(x)
====
