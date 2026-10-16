---------------- MODULE Fairness ----------------
VARIABLES x, g
vars == <<x,g>>
Init == /\ x = FALSE /\ g = TRUE
HostNext == g' = ~g /\ UNCHANGED x
HostSpec == Init /\ [][HostNext]_vars /\ WF_vars(HostNext)
I == INSTANCE FairInner WITH y <- x, gate <- g
P == I!Weak
================================================
