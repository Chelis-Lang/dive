---- MODULE TfNestedFails ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Identity(p) == p
Relay(p) == LET Saved == p
                           Local(q) == Identity(q)
                      IN Local(Saved)
P == Relay(<>[]x)
====
