---- MODULE P_fair_string ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x="a";
begin L: x:="b"; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "e932e809")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = "a"
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = "b"
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
