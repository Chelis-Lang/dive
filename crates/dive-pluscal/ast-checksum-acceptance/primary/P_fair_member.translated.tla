---- MODULE P_fair_member ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x \in {0,1};
begin L: x:=1; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "678d049d")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x \in {0,1}
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
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
