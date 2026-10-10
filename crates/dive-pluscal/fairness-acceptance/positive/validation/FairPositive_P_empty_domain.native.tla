---- MODULE FairPositive_P_empty_domain ----
EXTENDS Naturals,TLC
(* --algorithm A variables g=0; fair process P \in{} variables x=0; begin L: + x:=1; E: - g:=x; end process; end algorithm; *)

VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({ })

Init == /\ g = 0
        /\ x = [self \in { } |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ x' = [x EXCEPT ![self] = 1]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ g' = g

E(self) == /\ pc[self] = "E"
           /\ g' = x[self]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ x' = x

P(self) == L(self) \/ E(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { }: P(self)) \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ \A self \in {} : WF_vars((pc[self] # "E") /\ P(self)) /\ SF_vars(L(self))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
