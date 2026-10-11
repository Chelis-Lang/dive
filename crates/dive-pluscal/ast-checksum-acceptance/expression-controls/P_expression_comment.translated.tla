---- MODULE P_expression_comment ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: x:= 1 (* inner *) + 2; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "1af34f3c" /\ chksum(tla) = "92143a58")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1             + 2
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
