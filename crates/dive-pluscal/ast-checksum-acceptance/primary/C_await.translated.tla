---- MODULE C_await ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x=0; { L: await x=0; x:=1; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "c878be1e" /\ chksum(tla) = "df6e047a")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x=0
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
