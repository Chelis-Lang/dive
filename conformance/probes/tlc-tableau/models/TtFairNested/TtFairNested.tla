---- MODULE TtFairNested ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Identity(p) == p
Live == Identity([](x => <>[]x))
Spec == Init /\ [][Next]_x /\ WF_x(Next) /\ SF_x(Next)
====
