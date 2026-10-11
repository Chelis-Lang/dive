---- MODULE MultipleProcedure_C_tail_parallel_arguments ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,b=1)

{ F:if(a>0){ call First(a-1,a); return; }else{ x:=x+b; }; R:return; };
process (One = 1)

{ L:call First(1,3); E:skip; }
process (Two = 2)

{ M:call First(2,4); N:skip; }
} *)

VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

ProcSet == {1} \cup {2}

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ b = [self \in ProcSet |-> 1]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> CASE self = 1 -> "L" [] self = 2 -> "M"]

F(self) == /\ pc[self] = "F"
           /\ IF a[self] > 0
                 THEN /\ /\ a' = [a EXCEPT ![self] = a[self] - 1]
                         /\ b' = [b EXCEPT ![self] = a[self]]
                      /\ pc' = [pc EXCEPT ![self] = "F"]
                      /\ x' = x
                 ELSE /\ x' = x + b[self]
                      /\ pc' = [pc EXCEPT ![self] = "R"]
                      /\ UNCHANGED << a, b >>
           /\ stack' = stack

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ b' = [b EXCEPT ![self] = Head(stack[self]).b]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ /\ a' = [a EXCEPT ![1] = 1]
        /\ b' = [b EXCEPT ![1] = 3]
        /\ stack' = [stack EXCEPT ![1] = << [ procedure |-> "First", pc |-> "E", a |-> a[1], b |-> b[1] ] >> \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack, a, b >>

One == L \/ E

M == /\ pc[2] = "M"
     /\ /\ a' = [a EXCEPT ![2] = 2]
        /\ b' = [b EXCEPT ![2] = 4]
        /\ stack' = [stack EXCEPT ![2] = << [ procedure |-> "First", pc |-> "N", a |-> a[2], b |-> b[2] ] >> \o stack[2]]
     /\ pc' = [pc EXCEPT ![2] = "F"]
     /\ x' = x

N == /\ pc[2] = "N"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![2] = "Done"]
     /\ UNCHANGED << x, stack, a, b >>

Two == M \/ N

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == One \/ Two \/ (\E self \in ProcSet: First(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")

====
