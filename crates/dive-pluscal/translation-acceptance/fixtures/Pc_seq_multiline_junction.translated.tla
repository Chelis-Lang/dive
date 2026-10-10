---- MODULE Pc_seq_multiline_junction ----
EXTENDS Integers, Sequences, FiniteSets, TLC
(*
--algorithm A
variable x=0; begin L: print /\ TRUE
                            /\ x=0; E: skip; end algorithm;
*)
\* BEGIN TRANSLATION (chksum(pcal) = "3c154bfe" /\ chksum(tla) = "4bdacca4")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ PrintT( /\ TRUE
               /\ x=0)
     /\ pc' = "E"
     /\ x' = x

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
