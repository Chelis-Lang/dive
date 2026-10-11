---- MODULE C_expression_escaped ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= "a\t\n\"b"; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "5e7d0fa8" /\ chksum(tla) = "1dfbcdd3")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = "a\t\n\"b"
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
