---- MODULE C_shape_goto_done ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:=1; goto "Done"; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "39e59272" /\ chksum(tla) = "d14ef4d6")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
