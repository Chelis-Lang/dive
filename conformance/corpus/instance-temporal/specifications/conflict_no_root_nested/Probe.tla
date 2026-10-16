---------------- MODULE Probe ----------------
VARIABLES x
vars == <<x>>
Init == /\ x = FALSE
HostNext == UNCHANGED vars
HostSpec == Init /\ [][HostNext]_vars
DirectA == /\ x = FALSE /\ x' = FALSE /\ x' = TRUE
DirectCan == ENABLED DirectA
J == INSTANCE PairMiddle WITH q <- x
Q == J!Can
P == []~Q
================================================
