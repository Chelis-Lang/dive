---- MODULE LtAction ----
VARIABLE x
Init == x=FALSE
Live(p) == LET Now == p IN []<><<Now' # Now>>_Now
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == Live(x)
====
