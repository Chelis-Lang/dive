---- MODULE C_fair_default_name ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0,defaultInitValue=9; process(P = 1){L:g:=1; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "25c05565")
VARIABLES pc, g, defaultInitValue

vars == << pc, g, defaultInitValue >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ defaultInitValue = 9
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ g' = 1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED defaultInitValue

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, defaultInitValue >>

P == L \/ E

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
