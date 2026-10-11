---- MODULE P_fair_line_split_cli_default ----
EXTENDS Integers,TLC,Sequences
\* PlusCal
\* options (-wf)
(* --fair algorithm A variables g=0; process P\in {1,2} begin L:g:=1; E:skip; end process; end algorithm; *)
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
        /\ WF_vars(Next)

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
