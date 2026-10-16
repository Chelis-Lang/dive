---------------- MODULE Fairness ----------------
VARIABLES x, g
vars == <<x,g>>
Init == /\ x = FALSE /\ g = TRUE
HostNext == g' = ~g /\ UNCHANGED x
HostSpec == Init /\ [][HostNext]_vars /\ WF_vars(HostNext)
A == /\ g /\ x' = ~x /\ UNCHANGED g
P == SF_x(A)
================================================
