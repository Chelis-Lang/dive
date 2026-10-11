---- MODULE P_fair_assert ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables x=0;
begin L: assert x=0; x:=1; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "dbe069ae")
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

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
