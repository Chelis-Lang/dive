---- MODULE C_expression_member ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0; { L: x:= IF 0 ∈ {0} THEN 1 ELSE 0; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "b2e8bc04" /\ chksum(tla) = "d21c6626")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = IF 0 ∈ {0} THEN 1 ELSE 0
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
