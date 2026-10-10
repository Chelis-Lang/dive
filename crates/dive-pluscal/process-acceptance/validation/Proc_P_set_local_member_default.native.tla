---- MODULE Proc_P_set_local_member_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables g=0; process P \in{1,2} variables x\in{0,1}; begin L: g:=x; E: x:=0; end process; end algorithm; *)

VARIABLES pc, g, x

vars == << pc, g, x >>

ProcSet == ({ 1 , 2 })

Init == /\ g = 0
        /\ x \in [{ 1 , 2 } -> { 0 , 1 }]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ g' = x[self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ x' = x

E(self) == /\ pc[self] = "E"
           /\ x' = [x EXCEPT ![self] = 0]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ g' = g

P(self) == L(self) \/ E(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { 1 , 2 }: P(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
