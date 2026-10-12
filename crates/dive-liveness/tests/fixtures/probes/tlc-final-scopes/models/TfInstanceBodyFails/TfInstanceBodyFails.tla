---- MODULE TfInstanceBodyFails ----
---- MODULE Middle ----
---- MODULE Leaf ----
VARIABLE v
Claim == <>[]v
====
VARIABLE w
J == INSTANCE Leaf WITH v <- w
Claim == J!Claim
====
VARIABLE x
I == INSTANCE Middle WITH w <- x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
Identity(p) == p
P == Identity(I!Claim)
====
