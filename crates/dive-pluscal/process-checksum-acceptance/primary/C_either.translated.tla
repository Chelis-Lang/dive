---- MODULE C_either ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P = 1){L:either{g:=1;}or{g:=2;}; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "55e8626b" /\ chksum(tla) = "7c9e5b33")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ \/ /\ g' = 1
        \/ /\ g' = 2
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
