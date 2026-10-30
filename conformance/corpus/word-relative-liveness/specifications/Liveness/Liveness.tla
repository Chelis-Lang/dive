---- MODULE Liveness ----
EXTENDS Naturals
CONSTANT N, FAIR
VARIABLES x, y
vars == <<x, y>>
Init == x = 0 /\ y = 0
IncX == x < N /\ x' = x + 1 /\ UNCHANGED y
IncY == y < N /\ y' = y + 1 /\ UNCHANGED x
Reset == x = N /\ y = N /\ x' = 0 /\ y' = 0
Toggle == y' = (y + 1) % (N+1) /\ UNCHANGED x
Next == IncX \/ IncY \/ Reset \/ Toggle
Spec == Init /\ [][Next]_vars /\ WF_vars(IncY) /\ (IF FAIR = "sf" THEN SF_vars(IncX) ELSE IF FAIR = "wf" THEN WF_vars(IncX) ELSE TRUE)
XReachesN == <>(x = N)
AlwaysEvXN == []<>(x = N)
LeadsTo == (x = 1) ~> (x = N)
QuantProp == \A k \in 0..N : (x = k) ~> (x # k)
====
