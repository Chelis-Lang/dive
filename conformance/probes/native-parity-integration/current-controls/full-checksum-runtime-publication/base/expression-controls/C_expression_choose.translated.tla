---- MODULE C_expression_choose ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= CHOOSE z ∈ {1}: z=1; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "cd7ebd54" /\ chksum(tla) = "19d4c8fb")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (CHOOSE z ∈ {1}: z=1)
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
