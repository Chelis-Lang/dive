---- MODULE IsAction ----
---- MODULE Inner ----
CONSTANT C
VARIABLE v
Claim == []<><<v' # v>>_v
====
VARIABLE x
I == INSTANCE Inner WITH v <- x, C <- TRUE
Init == x=FALSE
Next == x'=~x
Spec == Init /\ [][Next]_x /\ WF_x(Next)
P == I!Claim
====
