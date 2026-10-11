---- MODULE FairProcedure_P_constants_reach_disjoint ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=x+a; R:return;
end procedure;
procedure Second(b=0)

begin
S:x:=x+b; T:return;
end procedure;
fair process One = 1

begin
L:call First(1); E:skip;
end process;
fair + process Two = 2

begin
M:call Second(2); N:skip;
end process;
end algorithm; *)

VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

ProcSet == {1} \cup {2}

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ b = [self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = 2 -> "M"]

F(self) == /\ pc[self] = "F"
           /\ x' = x + a[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a, b >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, b >>

First(self) == F(self) \/ R(self)

S(self) == /\ pc[self] = "S"
           /\ x' = x + b[self]
           /\ pc' = [pc EXCEPT ![self] = "T"]
           /\ UNCHANGED << stack, a, b >>

T(self) == /\ pc[self] = "T"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ b' = [b EXCEPT ![self] = Head(stack[self]).b]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ UNCHANGED << x, a >>

Second(self) == S(self) \/ T(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 1]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |-> "First", pc |-> "E", a |-> a[1] ] >> \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ UNCHANGED << x, b >>

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, b >>

One == L \/ E

M == /\ pc[2] = "M"
     /\ /\ b' = [b EXCEPT ![2] = 2]
        /\ stack' = [stack EXCEPT ![2] = << [ procedure |-> "Second", pc |-> "N", b |-> b[2] ] >> \o stack[2]]
     /\ pc' = [pc EXCEPT ![2] = "S"]
     /\ UNCHANGED << x, a >>

N == /\ pc[2] = "N"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack, a, b >>

Two == M \/ N

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ Two \/ (\E self \in ProcSet: First(self) \/ Second(self)) \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(One) /\ WF_vars(First(1))
        /\ SF_vars(Two) /\ SF_vars(Second(2))

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
