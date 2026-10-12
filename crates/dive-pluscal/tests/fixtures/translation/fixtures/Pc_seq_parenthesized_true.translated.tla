---- MODULE Pc_seq_parenthesized_true ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: while (TRUE) do x:=(x+1)%2; end while; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "c92790f3" /\ chksum(tla) = "fd8e33")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (x+1)%2
     /\ pc' = "L"

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
