---- MODULE MacroProcess_P_unused_foreign ----
EXTENDS Integers, TLC
(* --algorithm A variable x=0,y=0; macro Put(v,a) begin v:=a; end macro;macro Unused() begin z:=w; end macro; process P\in {1,2} variable z=0; begin L: Put(z,self); E: skip; end process;process Q\in {3,4} variable w=0; begin M: Put(w,self); F: skip; end process; end algorithm; *)

VARIABLES pc, x, y, z, w

vars == << pc, x, y, z, w >>

ProcSet == ({ 1 , 2 }) \cup ({ 3 , 4 })

Init == /\ x = 0
        /\ y = 0
        /\ z = [self \in { 1 , 2 } |-> 0]
        /\ w = [self \in { 3 , 4 } |-> 0]
        /\ pc = [self \in ProcSet |-> CASE self \in { 1 , 2 } -> "L" [] self \in { 3 , 4 } -> "M"]

L(self) == /\ pc[self] = "L"
           /\ z' = [z EXCEPT ![self] = self]
           /\ pc' = [pc EXCEPT ![self] = "E"]
           /\ UNCHANGED << x, y, w >>

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, y, z, w >>

P(self) == L(self) \/ E(self)

M(self) == /\ pc[self] = "M"
           /\ w' = [w EXCEPT ![self] = self]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ UNCHANGED << x, y, z >>

F(self) == /\ pc[self] = "F"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, y, z, w >>

Q(self) == M(self) \/ F(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in { 1 , 2 }: P(self)) \/ (\E self \in { 3 , 4 }: Q(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
