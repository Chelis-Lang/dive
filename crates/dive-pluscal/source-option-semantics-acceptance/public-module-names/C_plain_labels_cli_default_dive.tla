---- MODULE C_plain_labels_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-label, -labelRoot Src)
(* --algorithm A {variables g=0; process(P\in {1,2}){g:=1;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "ca981b53" /\ chksum(tla) = "450bcf36")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({ 1 , 2 })

Init == /\ g = 0
        /\ pc = [self \in ProcSet |-> "Src1"]

Src1(self) == /\ pc[self] = "Src1"
              /\ g' = 1
              /\ pc' = [pc EXCEPT ![self] = "Done"]

P(self) == Src1(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { 1 , 2 }: P(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")
\* END TRANSLATION 
====
