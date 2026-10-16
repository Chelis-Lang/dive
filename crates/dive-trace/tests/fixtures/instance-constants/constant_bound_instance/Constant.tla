---------------- MODULE Constant ----------------
EXTENDS Shared
VARIABLE x
vars == <<x>>
Table == [b \in BOOLEAN |-> ~b]
OtherTable == [b \in BOOLEAN |-> b]
Init == x = FALSE
I == INSTANCE ConstInner WITH y <- x, C <- OtherTable
HostNext == I!A
HostSpec == Init /\ [][HostNext]_vars /\ WF_vars(HostNext)
P == []<>(x = FALSE)
================================================
