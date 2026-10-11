---- MODULE C_fair_context_variable_label ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables L=0; {L:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "bb0f8d6")
\* Label L at line 3 col 42 changed to L_
VARIABLES pc, L

vars == << pc, L >>

Init == (* Global variables *)
        /\ L = 0
        /\ pc = "L_"

L_ == /\ pc = "L_"
      /\ TRUE
      /\ pc' = "Done"
      /\ L' = L

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L_
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
