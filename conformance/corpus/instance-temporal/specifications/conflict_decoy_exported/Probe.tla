---------------- MODULE Probe ----------------
VARIABLES x,y,z
vars == <<x,y,z>>
Init == /\ x = FALSE /\ y = TRUE /\ z = FALSE
HostNext == UNCHANGED vars
HostSpec == Init /\ [][HostNext]_vars
DirectA == /\ x = FALSE /\ x' = FALSE /\ x' = TRUE
DirectCan == ENABLED DirectA
I == INSTANCE Pair WITH y <- x, z <- x
Q == I!Can
P == []~Q
================================================
