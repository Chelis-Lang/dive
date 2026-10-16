---------------- MODULE Probe ----------------
VARIABLES x,y
vars == <<x,y>>
Init == /\ x = FALSE /\ y = TRUE
HostNext == x' = ~x /\ UNCHANGED <<y>>
HostSpec == Init /\ [][HostNext]_vars
DirectA == /\ x = FALSE /\ x' = TRUE
DirectCan == ENABLED DirectA
I == INSTANCE Guard WITH y <- x
Q == I!Can
P == [](Q <=> (x = FALSE))
================================================
