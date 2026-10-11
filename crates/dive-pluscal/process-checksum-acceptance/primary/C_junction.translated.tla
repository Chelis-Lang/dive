---- MODULE C_junction ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1){L:await /\ TRUE
 /\ g=0; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "a7367d97" /\ chksum(tla) = "52d8c45e")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\                                                        /\ TRUE
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

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
