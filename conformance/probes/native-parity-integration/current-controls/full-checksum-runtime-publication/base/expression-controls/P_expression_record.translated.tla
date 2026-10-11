---- MODULE P_expression_record ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: x:= [a |-> 1].a; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "dc7f98df" /\ chksum(tla) = "d7c40d7e")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = [a |-> 1].a
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
