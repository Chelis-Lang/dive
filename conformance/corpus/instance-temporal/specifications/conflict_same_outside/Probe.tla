---------------- MODULE Probe ----------------
VARIABLES y
vars == <<y>>
Init == /\ y = FALSE
HostNext == UNCHANGED vars
HostSpec == Init /\ [][HostNext]_vars
DirectA == /\ y = FALSE /\ y' = FALSE /\ y' = TRUE
DirectCan == ENABLED DirectA
I == INSTANCE Pair WITH y <- y, z <- y
Q == ENABLED I!A
P == []~Q
================================================
