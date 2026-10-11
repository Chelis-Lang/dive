---- MODULE Define_P_process_local ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == z
 end define;
begin L: x := F; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "6aaf28c4" /\ chksum(tla) = "6e655f70")
VARIABLES pc, x, y

(* define statement *)
F == z


vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = F
     /\ pc' = "Done"
     /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
