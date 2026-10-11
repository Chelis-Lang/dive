---- MODULE ProcessProcedure_C_two_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+1,v=u+1;
{ F:x:=u+v; R:return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)

VARIABLES pc, x, stack, a, u, v

vars == << pc, x, stack, a, u, v >>

ProcSet == {1}

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ u = [self \in ProcSet |-> a[self] + 1]
        /\ v = [self \in ProcSet |-> u[self] + 1]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ x' = u[self] + v[self]
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ UNCHANGED << stack, a, u, v >>

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ u' = [u EXCEPT ![self] = Head(stack[self]).u]
           /\ v' = [v EXCEPT ![self] = Head(stack[self]).v]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 2]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |-> "First", pc |-> "E", u |-> u[1], v |-> v[1], a |-> a[1] ] >> \o stack[1]]
     /\ u' = [u EXCEPT ![1] = a'[1] + 1]
     /\ v' = [v EXCEPT ![1] = u'[1] + 1]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, u, v >>

Worker == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Worker \/ (\E self \in ProcSet: First(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
