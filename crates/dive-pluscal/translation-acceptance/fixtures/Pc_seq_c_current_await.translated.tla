---- MODULE Pc_seq_c_current_await ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
{variable x=0; { L: x:=1; await x=1; E: skip; }}
*)
\* BEGIN TRANSLATION (chksum(pcal) = "38d4e722" /\ chksum(tla) = "c83e7398")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ x'=1
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
