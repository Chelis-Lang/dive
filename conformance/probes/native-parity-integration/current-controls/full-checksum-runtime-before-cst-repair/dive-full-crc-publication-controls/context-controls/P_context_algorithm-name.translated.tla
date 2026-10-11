---- MODULE P_context_algorithm_name ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables A=0; begin L:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "ef6d3321" /\ chksum(tla) = "4d34080e")
VARIABLES pc, A

vars == << pc, A >>

Init == (* Global variables *)
        /\ A = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ TRUE
     /\ pc' = "Done"
     /\ A' = A

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
