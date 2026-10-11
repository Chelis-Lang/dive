---- MODULE C_shape_if_no_else ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: if (x=0) { x:=1; } } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "5c5ff3aa" /\ chksum(tla) = "3f3695bf")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ x' = 1
           ELSE /\ TRUE
                /\ x' = x
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
