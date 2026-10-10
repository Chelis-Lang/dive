---- MODULE Pc_seq_comparison_rhs ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=FALSE; begin L: x:=1=1; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "2a260068" /\ chksum(tla) = "b2d44006")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = FALSE
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (1=1)
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
