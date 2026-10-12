---- MODULE Pc_seq_empty_variables ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
begin L: skip; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "b6a31834" /\ chksum(tla) = "1dae6401")
VARIABLE pc

vars == << pc >>

Init == /\ pc = "L"

L == /\ pc = "L"
     /\ TRUE
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
