---- MODULE P_goto ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0;
begin L: x:=1; goto L; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "51648c35" /\ chksum(tla) = "8250445b")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "L"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
