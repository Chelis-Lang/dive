---- MODULE P_fair_parallel ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A variables g=0; process P = 1 variables z=0; begin L:g:=z+1 || z:=g+1; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "cd82c9e9")
VARIABLES pc, g, z

vars == << pc, g, z >>

ProcSet == {1}

Init == (* Global variables *)
        /\ g = 0
        (* Process P *)
        /\ z = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ /\ g' = z+1
        /\ z' = g+1
     /\ pc' = [pc EXCEPT ![1] = "E"]

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << g, z >>

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
