---- MODULE MultipleProcedure_C_membership_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{ F:x:=x+a; R:return; };
process (One = 1)
variable y \in {1,2};
{ L:call First(1); E:skip; }
process (Two \in {2})
variable z \in {1,2};
{ M:call First(2); N:skip; }
} *)

VARIABLES pc, x, stack, a, y, z

vars == << pc, x, stack, a, y, z >>

ProcSet == {1} \cup ({ 2 })

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ y \in { 1 , 2 }
        /\ z \in [{ 2 } -> { 1 , 2 }]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self \in { 2 } -> "M"]

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

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 1]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |-> "First", pc |-> "E", a |-> a[1] ] >> \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ UNCHANGED << x, y, z >>

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, y, z >>

One == L \/ E

M(self) == /\ pc[self] = "M"
           /\ /\ a' = [a EXCEPT ![self] = 2]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |-> "First", pc |-> "N", a |-> a[self] ] >> \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ UNCHANGED << x, y, z >>

N(self) == /\ pc[self] = "N"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack, a, y, z >>

Two(self) == M(self) \/ N(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ (\E self \in ProcSet: First(self)) \/ (\E self \in { 2 }: Two(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
