---- MODULE IsNested ----
---- MODULE Middle ----
---- MODULE Leaf ----
CONSTANT C
VARIABLE v
Claim == []<>(v=C)
====
CONSTANT MC
VARIABLE w
J == INSTANCE Leaf WITH C <- MC, v <- w
Claim == J!Claim
====
VARIABLE x
I == INSTANCE Middle WITH MC <- TRUE, w <- x
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == I!Claim
====
