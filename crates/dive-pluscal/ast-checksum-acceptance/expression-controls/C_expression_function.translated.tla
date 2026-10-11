---- MODULE C_expression_function ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= [z \in {1} |-> z][1]; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "250e5940" /\ chksum(tla) = "1e453b8c")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [z \in {1} |-> z][1]
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
