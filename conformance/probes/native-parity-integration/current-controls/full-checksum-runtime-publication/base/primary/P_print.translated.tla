---- MODULE P_print ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0;
begin L: print x; x:=1; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "b331fc6c" /\ chksum(tla) = "e3d78140")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ PrintT(x)
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
