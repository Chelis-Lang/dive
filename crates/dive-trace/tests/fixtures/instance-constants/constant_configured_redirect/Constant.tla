---------------- MODULE Constant ----------------
EXTENDS Shared
VARIABLE x
vars == <<x>>
Table == [b \in BOOLEAN |-> ~b]
OtherTable == [b \in BOOLEAN |-> b]
Init == x = FALSE
Unused == INSTANCE ConstInner WITH y <- x
HostNext == x' = C[x]
HostSpec == Init /\ [][HostNext]_vars /\ WF_vars(HostNext)
P == []<>(x = FALSE)
AliasTable == Table
================================================
