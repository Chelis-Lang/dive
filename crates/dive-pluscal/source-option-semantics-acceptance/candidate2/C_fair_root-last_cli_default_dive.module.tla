---- MODULE C_fair_root_last_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal options (-labelRoot One, -labelRoot Two, -label)
(* --fair algorithm A {variables g=0; process(P\in {1,2}){g:=1;}} *)
VARIABLES pc, g

vars == << pc, g >>

ProcSet == ({ 1 , 2 })

Init == /\ g = 0
        /\ pc = [self \in ProcSet |-> "Two1"]

Two1(self) == /\ pc[self] = "Two1"
              /\ g' = 1
              /\ pc' = [pc EXCEPT ![self] = "Done"]

P(self) == Two1(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { 1 , 2 }: P(self)) \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
