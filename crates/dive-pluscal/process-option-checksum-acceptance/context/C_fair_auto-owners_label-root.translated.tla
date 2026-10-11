---- MODULE C_fair_auto_owners_label_root ----
EXTENDS Integers, TLC, Sequences
(* --fair algorithm A {variables g=0; process(P=1){g:=1; g:=2;} process(Q \in {2,3}){g:=1; g:=2;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7c28162a" /\ chksum(tla) = "8d0404d7")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == {1} \cup ({2,3})

Init == (* Global variables *)
        /\ g = 0
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "Auto1"
                                        [] self \in {2,3} -> "Auto3"]

Auto1 == /\ pc[1] = "Auto1"
         /\ g' = 1
         /\ pc' = [pc EXCEPT ![1] = "Auto2"]

Auto2 == /\ pc[1] = "Auto2"
         /\ g' = 2
         /\ pc' = [pc EXCEPT ![1] = "Done"]

P == Auto1 \/ Auto2

Auto3(self) == /\ pc[self] = "Auto3"
               /\ g' = 1
               /\ pc' = [pc EXCEPT ![self] = "Auto4"]

Auto4(self) == /\ pc[self] = "Auto4"
               /\ g' = 2
               /\ pc' = [pc EXCEPT ![self] = "Done"]

Q(self) == Auto3(self) \/ Auto4(self)

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == P
           \/ (\E self \in {2,3}: Q(self))
           \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

\* END TRANSLATION 
====
