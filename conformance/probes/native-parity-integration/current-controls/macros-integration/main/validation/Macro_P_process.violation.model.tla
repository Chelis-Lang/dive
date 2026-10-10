---- MODULE Macro_P_process ----
EXTENDS Integers, TLC
(* --algorithm A variables x=0, y=0; macro Put(v, a) begin v := a; end macro; process P \in {1,2} variable z=0; begin L: Put(z,self); E: skip; end process; end algorithm; *)

VARIABLES pc, x, y, z

vars == << pc, x, y, z >>

ProcSet == ({ 1 , 2 })

Init == /\ x = 0
        /\ y = 0
        /\ z = [self \in { 1 , 2 } |-> 0]
        /\ pc = [self \in ProcSet |-> "L"]

L(self) == /\ pc[self] = "L"
           /\ z' = [z EXCEPT ![self] = self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ UNCHANGED << x, y >>

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, y, z >>

P(self) == L(self) \/ E(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { 1 , 2 }: P(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")


Good == \A self \in ProcSet: pc[self] # "E"
====
