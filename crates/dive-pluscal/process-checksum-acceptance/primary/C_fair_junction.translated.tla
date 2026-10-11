---- MODULE C_fair_junction ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1){L:await /\ TRUE
 /\ g=0; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "a64c8824")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\                                                             /\ TRUE
        /\ g=0
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ g' = g

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ g' = g

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
