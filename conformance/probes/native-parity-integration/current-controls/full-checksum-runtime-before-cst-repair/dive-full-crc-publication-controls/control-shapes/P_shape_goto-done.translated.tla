---- MODULE P_shape_goto_done ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: x:=1; goto "Done"; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "39e59272" /\ chksum(tla) = "d14ef4d6")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
