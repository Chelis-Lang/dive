---- MODULE ProcessProcedure_C_self_recursive_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+1;
{ F:if(a>0){ call First(a-1); }else{ x:=u; }; R:x:=x+u; return; };

process (Worker = 1)

{ L:call First(2); E:skip; }
} *)

VARIABLES pc, x, stack, a, u

vars == << pc, x, stack, a, u >>

ProcSet == {1}

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ u = [self \in ProcSet |-> a[self] + 1]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ IF a[self] > 0
                 THEN /\ /\ a' = [a EXCEPT ![self] = a[self] - 1]
                         /\ stack' = [stack EXCEPT ![self] = << [ procedure |-> "First", pc |-> "R", u |-> u[self], a |-> a[self] ] >> \o stack[self]]
                      /\ u' = [u EXCEPT ![self] = a'[self] + 1]
                      /\ pc' = [pc EXCEPT ![self] = "F"]
                      /\ x' = x
                 ELSE /\ x' = u[self]
                      /\ pc' = [pc EXCEPT ![self] = "R"]
                      /\ UNCHANGED << stack, a, u >>

R(self) == /\ pc[self] = "R"
           /\ x' = x + u[self]
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ u' = [u EXCEPT ![self] = Head(stack[self]).u]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 2]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |-> "First", pc |-> "E", u |-> u[1], a |-> a[1] ] >> \o stack[1]]
     /\ u' = [u EXCEPT ![1] = a'[1] + 1]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, u >>

Worker == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Worker \/ (\E self \in ProcSet: First(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
