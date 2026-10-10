---- MODULE MultipleProcedure_P_member_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One \in {1}
variable y=self;
begin
L:call First(y); E:y:=x;
end process;
process Two \in {2}
variable z=self;
begin
M:call First(z); N:z:=x;
end process;
end algorithm; *)

VARIABLES pc, x, stack, a, y, z

vars == << pc, x, stack, a, y, z >>

ProcSet == ({ 1 }) \cup ({ 2 })

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ y = [self \in { 1 } |-> self]
        /\ z = [self \in { 2 } |-> self]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self \in { 1 } -> "L" [] self \in { 2 } -> "M"]

F(self) == /\ pc[self] = "F"
           /\ x' = x + a[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a, y, z >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, y, z >>

First(self) == F(self) \/ R(self)

L(self) == /\ pc[self] = "L"
           /\ /\ a' = [a EXCEPT ![self] = y[self]]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |-> "First", pc |-> "E", a |-> a[self] ] >> \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ UNCHANGED << x, y, z >>

E(self) == /\ pc[self] = "E"
           /\ y' = [y EXCEPT ![self] = x]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack, a, z >>

One(self) == L(self) \/ E(self)

M(self) == /\ pc[self] = "M"
           /\ /\ a' = [a EXCEPT ![self] = z[self]]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |-> "First", pc |-> "N", a |-> a[self] ] >> \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ UNCHANGED << x, y, z >>

N(self) == /\ pc[self] = "N"
           /\ z' = [z EXCEPT ![self] = x]
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack, a, y >>

Two(self) == M(self) \/ N(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in ProcSet: First(self)) \/ (\E self \in { 1 }: One(self)) \/ (\E self \in { 2 }: Two(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
