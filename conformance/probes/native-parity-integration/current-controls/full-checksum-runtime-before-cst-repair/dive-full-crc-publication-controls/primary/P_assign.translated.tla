---- MODULE P_assign ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0;
begin L: x := x+1; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "3923bd05" /\ chksum(tla) = "2852b282")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = x+1
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
