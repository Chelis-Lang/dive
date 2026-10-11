---- MODULE P_fair_context_algorithm_name ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables A=0; begin L:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "24f6e7b1")
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

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
