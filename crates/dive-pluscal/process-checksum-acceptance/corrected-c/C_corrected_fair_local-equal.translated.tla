---- MODULE C_corrected_fair_local_equal ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1) variables z=0; {L:z:=z+1; E:g:=z;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "870c0b79")
VARIABLES pc, g, z

vars == << pc, g, z >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ z = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ z' = z+1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ g' = g

E == /\ pc[1] = "E"
     /\ g' = z
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ z' = z

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
