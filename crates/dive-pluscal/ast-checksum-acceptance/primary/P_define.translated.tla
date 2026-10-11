---- MODULE P_define ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; define Z==1 end define;
begin L:x:=Z; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "dc306b0a" /\ chksum(tla) = "7331a303")
VARIABLES pc, x

(* define statement *)
Z==1


vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = Z
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
