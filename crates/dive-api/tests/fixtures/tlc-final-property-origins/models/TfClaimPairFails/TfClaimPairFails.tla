---- MODULE TfClaimPairFails ----
VARIABLE x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Identity(p) == p
P == Identity(<>[]x)
Q == Identity(<>[]~x)
====
