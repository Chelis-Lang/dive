---------------- MODULE Fairness ----------------
VARIABLES x, g
vars == <<x,g>>
Init == /\ x = FALSE /\ g = TRUE
HostNext == UNCHANGED vars
HostSpec == Init /\ [][HostNext]_vars
I == INSTANCE FairInner WITH y <- x, gate <- g
P == I!Weak
================================================
