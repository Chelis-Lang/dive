---- MODULE FairStartupOutput ----
EXTENDS Naturals, TLC
VARIABLE x
ASSUME PrintT("startup")
Init == x=0
Step == x=0 /\ x'=1
Other == x=1 /\ x'=0
Next == Step \/ Other
Spec == Init /\ [][Next]_x /\ WF_x(Step)
P == <> (x=1)
====
