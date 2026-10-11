---- MODULE Define_P_escaped ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == "a\n"
 end define;
begin L: x := F; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "63f78299" /\ chksum(tla) = "50dc4141")
VARIABLES pc, x, y

(* define statement *)
F == "a\n"

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
