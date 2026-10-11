---- MODULE Ownership_C_identifier ----
EXTENDS Integers
(* --algorithm A { variable x=0; macro M(a) { x:=a; }; { L:M(1); E:skip; } } notes *)
\* BEGIN TRANSLATION (chksum(pcal) = "40fe0264" /\ chksum(tla) = "14608fc6")
VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
