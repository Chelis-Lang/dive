---- MODULE Pc_seq_either_goto ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: either goto E; or x:=1; end either; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "864daf81" /\ chksum(tla) = "7aaf0cb5")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ \/ /\ pc' = "E"
           /\ x' = x
        \/ /\ x' = 1
           /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
