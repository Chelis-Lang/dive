---- MODULE P_assert ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0;
begin L: assert x=0; x:=1; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "2e08062f" /\ chksum(tla) = "99f9bbb4")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ Assert(x=0, "Failure of assertion at line 4, column 10.")
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
