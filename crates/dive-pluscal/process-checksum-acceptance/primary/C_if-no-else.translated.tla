---- MODULE C_if_no_else ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1){L:if(g=0){g:=1;}; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "cd43784" /\ chksum(tla) = "e222d3e5")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ IF g=0
           THEN /\ g' = 1
           ELSE /\ TRUE
                /\ g' = g
     /\ pc' = [pc EXCEPT ![1] = "E"]

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
