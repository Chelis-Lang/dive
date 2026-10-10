---- MODULE Pc_seq_vars_conflict ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable vars=0; begin L: vars := 1; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "f2a92a6e" /\ chksum(tla) = "b6ac29ed")
VARIABLES pc, vars

vars == << pc, vars >>

Init == (* Global variables *)
        /\ vars = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ vars' = 1
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ vars' = vars

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
