---- MODULE ProcessProcedureExtra_C_member_parallel_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=1)

{ F:if(a>0){ call First(a-1,a); return; }else{ x:=c; }; R:return; };
process (Worker \in {1,2}) { L:call First(2,3); E:skip; }
} *)

VARIABLES pc, x, stack, a, c

vars == << pc, x, stack, a, c >>

ProcSet == ({ 1 , 2 })

Init == /\ x = 0
        /\ a = [self \in ProcSet |-> 0]
        /\ c = [self \in ProcSet |-> 1]
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ IF a[self] > 0
                 THEN /\ /\ a' = [a EXCEPT ![self] = a[self] - 1]
                         /\ c' = [c EXCEPT ![self] = a[self]]
                      /\ pc' = [pc EXCEPT ![self] = "F"]
                      /\ x' = x
                 ELSE /\ x' = c[self]
                      /\ pc' = [pc EXCEPT ![self] = "R"]
                      /\ UNCHANGED << a, c >>
           /\ stack' = stack

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ a' = [a EXCEPT ![self] = Head(stack[self]).a]
           /\ c' = [c EXCEPT ![self] = Head(stack[self]).c]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L(self) == /\ pc[self] = "L"
           /\ /\ a' = [a EXCEPT ![self] = 2]
              /\ c' = [c EXCEPT ![self] = 3]
              /\ stack' = [stack EXCEPT ![self] = << [ procedure |-> "First", pc |-> "E", a |-> a[self], c |-> c[self] ] >> \o stack[self]]
           /\ pc' = [pc EXCEPT ![self] = "F"]
           /\ x' = x

E(self) == /\ pc[self] = "E"
           /\ TRUE
           /\ pc' = [pc EXCEPT ![self] = "Done"]
           /\ UNCHANGED << x, stack, a, c >>

Worker(self) == L(self) \/ E(self)

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == (\E self \in ProcSet: First(self)) \/ (\E self \in { 1 , 2 }: Worker(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")


Good == \A self \in ProcSet: pc[self] # "E"
====
