---- MODULE MultipleProcedure_P_recursive_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=a;
begin
F:if a>0 then call First(a-1); else x:=x+u; end if; R:x:=x+u; return;
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

VARIABLES pc, x, stack, a, u

vars == << pc, x, stack, a, u >>

ProcSet == {1} \cup {2}

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ u = [self \in ProcSet |-> a[self]]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = 2 -> "M"]

F(self) == /\ pc[self] = "F"
           /\ IF a[self] > 0
                 THEN /\ /\ a' = [a EXCEPT ![self] = a[self] - 1]
                         /\ stack' = [stack EXCEPT ![self] = << [ procedure |-> "First", pc |-> "R", u |-> u[self], a |-> a[self] ] >> \o stack[self]]
                      /\ u' = [u EXCEPT ![self] = a'[self]]
                      /\ pc' = [pc EXCEPT ![self] = "F"]
                      /\ x' = x
                 ELSE /\ x' = x + u[self]
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
     /\ /\ a' = [a EXCEPT ![1] = 1]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |-> "First", pc |-> "E", u |-> u[1], a |-> a[1] ] >> \o stack[1]]
     /\ u' = [u EXCEPT ![1] = a'[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, u >>

One == L \/ E

M == /\ pc[2] = "M"
     /\ /\ a' = [a EXCEPT ![2] = 2]
        /\ stack' = [stack EXCEPT ![2] = << [ procedure |-> "First", pc |-> "N", u |-> u[2], a |-> a[2] ] >> \o stack[2]]
     /\ u' = [u EXCEPT ![2] = a'[2]]
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ x' = x

N == /\ pc[2] = "N"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack, a, u >>

Two == M \/ N

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ Two \/ (\E self \in ProcSet: First(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")


Good == \A self \in ProcSet: pc[self] # "E"
====
