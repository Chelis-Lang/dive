---- MODULE P_fair_define ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0; define Z==1 end define;
begin L:x:=Z; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "e832bd0b")
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

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
