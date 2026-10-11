---- MODULE P_process_lines ----
EXTENDS Integers,TLC,Sequences
(* --algorithm A variables x=0; process P = 1 variables z=0; begin L:z:=1+
 2; E:skip; end process; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "f2b9aefe" /\ chksum(tla) = "883eb364")
VARIABLES pc, x, z

vars == << pc, x, z >>

ProcSet == {1}

Init == (* Global variables *)
        /\ x = 0
        (* Process P *)
        /\ z = 0
        /\ pc = [self \in ProcSet |-> "L"]

L == /\ pc[1] = "L"
     /\ z' =                                                                        1+
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

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
