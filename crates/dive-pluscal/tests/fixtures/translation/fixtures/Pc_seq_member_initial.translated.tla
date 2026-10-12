---- MODULE Pc_seq_member_initial ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x \in 0..2; begin L: x := x+1; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "e212cfa9" /\ chksum(tla) = "1798046b")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x \in 0..2
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = x+1
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
