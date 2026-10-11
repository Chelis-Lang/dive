---- MODULE Define_P_temporal ----
EXTENDS Naturals, Integers, Sequences, TLC
(*
--algorithm A
 variables x=0, y=0;
 define
 F == [] (x=0)
 end define;
begin L: x := F; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "da2dd3ed" /\ chksum(tla) = "eeaf97dc")
VARIABLES pc, x, y

(* define statement *)
F == [] (x=0)


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
