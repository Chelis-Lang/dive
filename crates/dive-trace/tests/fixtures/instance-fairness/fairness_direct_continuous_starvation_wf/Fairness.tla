---------------- MODULE Fairness ----------------
VARIABLES x, g
vars == <<x,g>>
Init == /\ x = FALSE /\ g = TRUE
HostNext == UNCHANGED vars
HostSpec == Init /\ [][HostNext]_vars
A == /\ g /\ x' = ~x /\ UNCHANGED g
P == WF_x(A)
================================================
