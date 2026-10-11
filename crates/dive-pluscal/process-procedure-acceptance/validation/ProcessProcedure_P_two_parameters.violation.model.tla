---- MODULE ProcessProcedure_P_two_parameters ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,c=a+1)

begin
F:x:=a+c; R:return;
end procedure;

process Worker = 1

begin
L:call First(2,3); E:skip;
end process;
end algorithm; *)

VARIABLES pc, x, stack, a, c

vars == << pc, x, stack, a, c >>

ProcSet == {1}

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ c = [self \in ProcSet |-> a[self] + 1]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ x' = a[self] + c[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a, c >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ c' = [c EXCEPT ![self] = Head(stack[self]).c]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 2]
        /\ c' = [c EXCEPT ![1] = 3]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |-> "First", pc |-> "E", a |-> a[1], c |-> c[1] ] >> \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, c >>

Worker == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Worker \/ (\E self \in ProcSet: First(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")


Good == \A self \in ProcSet: pc[self] # "E"
====
