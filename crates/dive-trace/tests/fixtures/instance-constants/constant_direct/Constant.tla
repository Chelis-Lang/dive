---------------- MODULE Constant ----------------
EXTENDS Shared
VARIABLE x
vars == <<x>>
Table == [b \in BOOLEAN |-> ~b]
OtherTable == [b \in BOOLEAN |-> b]
Init == x = FALSE
HostNext == x' = C[x]
HostSpec == Init /\ [][HostNext]_vars /\ WF_vars(HostNext)
P == []<>(x = FALSE)
================================================
