---- MODULE C_fair_parallel ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A { variables x=0,y=1; { L: x:=y || y:=x; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "7e299564")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 1
        /\ pc = "L"

L == /\ pc = "L"
     /\ /\ x' = y
        /\ y' = x
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
