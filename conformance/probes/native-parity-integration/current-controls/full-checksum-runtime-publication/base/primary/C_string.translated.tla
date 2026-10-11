---- MODULE C_string ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x="a"; { L: x:="b"; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "3ee15138" /\ chksum(tla) = "5b1fc8a4")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = "a"
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = "b"
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
