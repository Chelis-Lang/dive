---------------- MODULE Fairness ----------------
VARIABLES x, g
vars == <<x,g>>
Init == /\ x = FALSE /\ g = FALSE
HostNext == UNCHANGED vars
HostSpec == Init /\ [][HostNext]_vars
A == /\ g /\ x' = ~x /\ UNCHANGED g
P == SF_x(A)
================================================
