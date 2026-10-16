---------------- MODULE Fairness ----------------
VARIABLES x, g
vars == <<x,g>>
Init == /\ x = FALSE /\ g = TRUE
HostNext == x' = ~x /\ UNCHANGED g
HostSpec == Init /\ [][HostNext]_vars /\ WF_vars(HostNext)
I == INSTANCE FairInner WITH y <- x, gate <- g
P == I!Strong
================================================
