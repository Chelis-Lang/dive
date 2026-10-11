---- MODULE P_junction ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0;
begin L: await /\ TRUE /\ x=0; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "bd1bf589" /\ chksum(tla) = "3e1a980f")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ /\ TRUE /\ x=0
     /\ pc' = "Done"
     /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
