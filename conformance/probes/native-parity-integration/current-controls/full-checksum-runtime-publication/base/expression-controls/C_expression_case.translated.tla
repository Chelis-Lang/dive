---- MODULE C_expression_case ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= CASE TRUE -> 1 [] OTHER -> 0; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "6114eded" /\ chksum(tla) = "5e3deb5a")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (CASE TRUE -> 1 [] OTHER -> 0)
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
