---- MODULE P_expression_proof_next ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0; begin L: x:= (1 = <<1>>); end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "69287902" /\ chksum(tla) = "5a80b81a")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (1 = <<1>>)
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
