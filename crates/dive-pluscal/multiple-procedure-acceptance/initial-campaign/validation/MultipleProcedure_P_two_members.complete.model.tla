---- MODULE MultipleProcedure_P_two_members ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
process One \in {1,2}

begin
L:call First(1); E:skip;
end process;
process Two \in {3,4}

begin
M:call First(2); N:skip;
end process;
end algorithm; *)

VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

ProcSet == ({ 1 , 2 }) \cup ({ 3 , 4 })

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self \in { 1 , 2 } -> "L" [] self \in { 3 , 4 } -> "M"]

F(self) == /\ pc[self] = "F"
           /\ x' = x + a[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L(self) == /\ pc[self] = "L"
           /\ /\ a' = [a EXCEPT ![self] = 1]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |-> "First", pc |-> "E", a |-> a[self] ] >> \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ x' = x

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack, a >>

One(self) == L(self) \/ E(self)

M(self) == /\ pc[self] = "M"
           /\ /\ a' = [a EXCEPT ![self] = 2]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |-> "First", pc |-> "N", a |-> a[self] ] >> \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ x' = x

N(self) == /\ pc[self] = "N"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack, a >>

Two(self) == M(self) \/ N(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in ProcSet: First(self)) \/ (\E self \in { 1 , 2 }: One(self)) \/ (\E self \in { 3 , 4 }: Two(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
