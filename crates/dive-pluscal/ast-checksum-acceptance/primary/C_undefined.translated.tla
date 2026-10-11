---- MODULE C_undefined ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x=0; { L: x:=q; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "1116a9df" /\ chksum(tla) = "58a01936")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = q
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
