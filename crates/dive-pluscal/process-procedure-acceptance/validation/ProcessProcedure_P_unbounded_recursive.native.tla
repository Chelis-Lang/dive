---- MODULE ProcessProcedure_P_unbounded_recursive ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call First(a+1); R:return;
end procedure;

process Worker = 1

begin
L:call First(2); E:skip;
end process;
end algorithm; *)

VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

ProcSet == {1}

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ /\ a' = [a EXCEPT ![self] = a[self] + 1]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |-> "First", pc |-> "R", a |-> a[self] ] >> \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ x' = x

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 2]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |-> "First", pc |-> "E", a |-> a[1] ] >> \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a >>

Worker == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Worker \/ (\E self \in ProcSet: First(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
