---- MODULE C_parallel ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x=0,y=1; { L: x:=y || y:=x; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "32d790f2" /\ chksum(tla) = "cfa16e6b")
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

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
