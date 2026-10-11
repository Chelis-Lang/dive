---- MODULE P_if ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0;
begin L: if x=0 then x:=1; else skip; end if; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "cf9a30e6" /\ chksum(tla) = "3f3695bf")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ x' = 1
           ELSE /\ TRUE
                /\ x' = x
     /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
