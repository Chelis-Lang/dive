---- MODULE P_member ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x \in {0,1};
begin L: x:=1; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "8de8bcc2" /\ chksum(tla) = "715b4c00")
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

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
