---------------- MODULE Probe ----------------
VARIABLES x
vars == <<x>>
Init == /\ x = FALSE
HostNext == UNCHANGED vars
HostSpec == Init /\ [][HostNext]_vars
I == INSTANCE Rebind WITH y <- x, z <- x
Q == I!CanOld
P == []~Q
================================================
