---------------- MODULE Probe ----------------
VARIABLES x
vars == <<x>>
Init == /\ x = FALSE
HostNext == UNCHANGED vars
HostSpec == Init /\ [][HostNext]_vars
DirectA == /\ x = FALSE /\ x' = FALSE /\ x' = TRUE
DirectCan == ENABLED DirectA
I == INSTANCE Pair WITH y <- x, z <- x
Q == I!Can
P == []~Q
================================================
