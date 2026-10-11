---- MODULE P_expression_conjunction ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: x:= IF TRUE ∧ FALSE THEN 1 ELSE 0; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "21cff1a5" /\ chksum(tla) = "c3f50d96")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = IF TRUE ∧ FALSE THEN 1 ELSE 0
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
