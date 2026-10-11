---- MODULE C_expression_let ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= LET z == 1 IN z; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "625787cf" /\ chksum(tla) = "1cb99b00")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (LET z == 1 IN z)
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
