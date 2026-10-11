---- MODULE Define_P_string ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == "ASCII"
 end define;
begin L: x := F; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "52ddcc2b" /\ chksum(tla) = "d7164430")
VARIABLES pc, x, y

(* define statement *)
F == "ASCII"

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = F
     /\ pc' = "Done"
     /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
