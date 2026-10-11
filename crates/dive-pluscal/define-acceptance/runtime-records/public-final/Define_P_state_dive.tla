---- MODULE Define_P_state ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == x
 end define;
begin L: x := 1; y := F; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "867ec86a" /\ chksum(tla) = "c630c5b7")
VARIABLES pc, x, y

(* define statement *)
F == x

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ y' = F
     /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 
====
