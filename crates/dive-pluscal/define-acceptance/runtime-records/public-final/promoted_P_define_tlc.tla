---- MODULE P_define ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; define Z==1 end define;
begin L:x:=Z; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "dc306b0a" /\ chksum(tla) = "bc41d375")
VARIABLES pc, x

(* define statement *)
Z==1

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = Z
     /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
