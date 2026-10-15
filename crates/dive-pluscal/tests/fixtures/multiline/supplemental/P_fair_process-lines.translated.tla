---- MODULE P_fair_process_lines ----
EXTENDS Integers,TLC,Sequences
(* --fair algorithm A variables x=0; process P = 1 variables z=0; begin L:z:=1+
 2; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "126d99d")
VARIABLES pc, x, z

vars == << pc, x, z >>

ProcSet == {1}

Init == (* Global variables *)
        /\ x = 0
        (* Process P *)
        /\ z = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ z' =                                                                             1+
             2
     /\ pc' = [pc EXCEPT ![1] = "E"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, z >>

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
