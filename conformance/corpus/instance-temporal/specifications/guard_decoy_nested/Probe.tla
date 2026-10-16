---------------- MODULE Probe ----------------
VARIABLES x,y
vars == <<x,y>>
Init == /\ x = FALSE /\ y = TRUE
HostNext == x' = ~x /\ UNCHANGED <<y>>
HostSpec == Init /\ [][HostNext]_vars
DirectA == /\ x = FALSE /\ x' = TRUE
DirectCan == ENABLED DirectA
J == INSTANCE GuardMiddle WITH q <- x
Q == J!Can
P == [](Q <=> (x = FALSE))
================================================
