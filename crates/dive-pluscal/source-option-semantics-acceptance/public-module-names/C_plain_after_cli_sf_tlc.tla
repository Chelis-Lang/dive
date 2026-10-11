---- MODULE C_plain_after_cli_sf ----
EXTENDS Integers,TLC,Sequences

(* --algorithm A {variables g=0; process(P\in {1,2}){L:g:=1; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "247b4187" /\ chksum(tla) = "989c1332")
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({ 1 , 2 })

Init == /\ g = 0
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = 1
           /\ pc' = [pc EXCEPT ![self] = "E"]

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ g' = g

P(self) == L(self) \/ E(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { 1 , 2 }: P(self)) \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {1,2} : WF_vars(P(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")
\* END TRANSLATION 
\* PlusCal options (-wf)
====
