---- MODULE TfInstanceHolds ----
---- MODULE Middle ----
---- MODULE Leaf ----
VARIABLE v
Identity(p) == p
Claim(p) == Identity(p)
====
VARIABLE w
J == INSTANCE Leaf WITH v <- w
Claim(p) == J!Claim(p)
====
VARIABLE x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
I == INSTANCE Middle WITH w <- x
P == I!Claim([]<>x)
====
