---- MODULE P_with ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0;
begin L: with a=1 do x:=a; end with; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "ceb20d5c" /\ chksum(tla) = "4fbcd2dd")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ LET a == 1 IN
          x' = a
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
