---- MODULE C_context_variable_label ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables L=0; {L:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "776edd39" /\ chksum(tla) = "cbdc8a76")
\* Label L at line 3 col 37 changed to L_
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

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
