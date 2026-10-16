---------------- MODULE FairInner ----------------
VARIABLES y, gate
vars == <<y,gate>>
A == /\ gate /\ y' = ~y /\ UNCHANGED gate
Weak == WF_y(A)
Strong == SF_y(A)
================================================
