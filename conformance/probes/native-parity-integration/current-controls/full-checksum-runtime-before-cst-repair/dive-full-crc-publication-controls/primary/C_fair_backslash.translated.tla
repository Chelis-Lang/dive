---- MODULE C_fair_backslash ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A { variables x={0}; { L: x := x \cup {1}; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "db068f23")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = {0}
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = (x \cup {1})
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
