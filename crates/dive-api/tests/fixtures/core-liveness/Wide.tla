---- MODULE Wide ----
\* A few thousand states; the word fails on a prefix when every level is checked.
EXTENDS Naturals
VARIABLES a, b
Init == a = 0 /\ b = 0
IncA == a' = (a + 1) % 60 /\ b' = b
IncB == b < 70 /\ b' = b + 1 /\ a' = a
Spec == Init /\ [][IncA \/ IncB]_<<a, b>> /\ WF_<<a, b>>(IncA)
P == <>(b = 70)
====
