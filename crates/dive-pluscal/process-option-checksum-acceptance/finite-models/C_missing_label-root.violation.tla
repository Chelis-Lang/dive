---- MODULE C_missing_label_root ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables g=0; process(P \in {1,2}){g:=1; g:=2;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "3613489b" /\ chksum(tla) = "f4dad50b")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({ 1 , 2 })

Init == /\ g = 0
        /\ pc = [self \in ProcSet |-> "Auto1"]

Auto1(self) == /\ pc[self] = "Auto1"
               /\ g' = 1
               /\ pc' = [pc EXCEPT ![self] = "Auto2"]

Auto2(self) == /\ pc[self] = "Auto2"
               /\ g' = 2
               /\ pc' = [pc EXCEPT ![self] = "Done"]

P(self) == Auto1(self) \/ Auto2(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { 1 , 2 }: P(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")
\* END TRANSLATION 

Good == g = 0
====
