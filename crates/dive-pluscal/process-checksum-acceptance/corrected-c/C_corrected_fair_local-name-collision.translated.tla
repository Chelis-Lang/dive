---- MODULE C_corrected_fair_local_name_collision ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P = 1) variables z=0; {L:g:=g+1; E:skip;} process(Q = 2) variables z=0; {M:z:=1; F:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "185b7de4")
\* Process variable z of process P at line 3 col 64 changed to z_
VARIABLES pc, g, z_, z

vars == << pc, g, z_, z >>

ProcSet == {1} \cup {2}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ z_ = 0
        (* Process Q *)
        /\ z = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L"
                                        [] self = 2 -> "M"]

L == /\ pc[1] = "L"
     /\ g' = g+1
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ UNCHANGED << z_, z >>

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, z_, z >>

P == L \/ E

M == /\ pc[2] = "M"
     /\ z' = 1
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ UNCHANGED << g, z_ >>

F == /\ pc[2] = "F"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << g, z_, z >>

Q == M \/ F

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P \/ Q
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
