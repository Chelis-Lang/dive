---- MODULE C_expression_less_equal ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= IF 0 ≤ 1 THEN 1 ELSE 0; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "608fbd1f" /\ chksum(tla) = "133fa277")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = IF 0 ≤ 1 THEN 1 ELSE 0
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
