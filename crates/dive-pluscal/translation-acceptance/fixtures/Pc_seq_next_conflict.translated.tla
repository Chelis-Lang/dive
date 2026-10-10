---- MODULE Pc_seq_next_conflict ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable Next=0; begin L: Next := 1; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "f1754afd" /\ chksum(tla) = "85108fce")
VARIABLES pc, Next

vars == << pc, Next >>

Init == (* Global variables *)
        /\ Next = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ Next' = 1
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ Next' = Next

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
