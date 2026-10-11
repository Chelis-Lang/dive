---- MODULE P_skip ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0;
begin L: skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "d50b505e" /\ chksum(tla) = "89820973")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ TRUE
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
