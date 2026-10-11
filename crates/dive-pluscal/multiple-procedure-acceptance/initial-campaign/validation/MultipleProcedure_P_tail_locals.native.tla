---- MODULE MultipleProcedure_P_tail_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=a+1,v=u+1;
begin
F:if a>0 then call First(a-1); return; else x:=x+u+v; end if; R:return;
end procedure;
process One = 1

begin
L:call First(1); E:skip;
end process;
process Two = 2

begin
M:call First(2); N:skip;
end process;
end algorithm; *)

VARIABLES pc, x, stack, a, u, v

vars == << pc, x, stack, a, u, v >>

ProcSet == {1} \cup {2}

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ u = [self \in ProcSet |-> a[self] + 1]
        /\ v = [self \in ProcSet |-> u[self] + 1]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = 2 -> "M"]

F(self) == /\ pc[self] = "F"
           /\ IF a[self] > 0
                 THEN /\ a' = [a EXCEPT ![self] = a[self] - 1]
                      /\ u' = [u EXCEPT ![self] = a'[self] + 1]
                      /\ v' = [v EXCEPT ![self] = u'[self] + 1]
                      /\ pc' = [pc EXCEPT ![self] = "F"]
                      /\ x' = x
                 ELSE /\ x' = x + u[self] + v[self]
                      /\ pc' = [pc EXCEPT ![self] = "R"]
                      /\ UNCHANGED << a, u, v >>
           /\ stack' = stack

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ u' = [u EXCEPT ![self] = Head(stack[self]).u]
           /\ v' = [v EXCEPT ![self] = Head(stack[self]).v]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 1]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |-> "First", pc |-> "E", u |-> u[1], v |-> v[1], a |-> a[1] ] >> \o stack[1]]
     /\ u' = [u EXCEPT ![1] = a'[1] + 1]
     /\ v' = [v EXCEPT ![1] = u'[1] + 1]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, u, v >>

One == L \/ E

M == /\ pc[2] = "M"
     /\ /\ a' = [a EXCEPT ![2] = 2]
        /\ stack' = [stack EXCEPT ![2] = << [ procedure |-> "First", pc |-> "N", u |-> u[2], v |-> v[2], a |-> a[2] ] >> \o stack[2]]
     /\ u' = [u EXCEPT ![2] = a'[2] + 1]
     /\ v' = [v EXCEPT ![2] = u'[2] + 1]
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ x' = x

N == /\ pc[2] = "N"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack, a, u, v >>

Two == M \/ N

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ Two \/ (\E self \in ProcSet: First(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
