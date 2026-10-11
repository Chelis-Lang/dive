---- MODULE C_assert ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A { variables x=0; { L: assert x=0; x:=1; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "2e08062f" /\ chksum(tla) = "f7b10d06")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ Assert(x=0, "Failure of assertion at line 3, column 40.")
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
