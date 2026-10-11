---- MODULE ProcessProcedure_C_no_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{ F:x:=1; R:return; };

process (Worker = 1)

{ L:call First(); E:skip; }
} *)

VARIABLES pc, x, stack

vars == << pc, x, stack >>

ProcSet == {1}

Init == /\ x = 0
        /\ stack = [self \in ProcSet |-> << >>]
        /\ pc = [self \in ProcSet |-> "L"]

F(self) == /\ pc[self] = "F"
           /\ x' = 1
           /\ pc' = [pc EXCEPT ![self] = "R"]
           /\ stack' = stack

R(self) == /\ pc[self] = "R"
           /\ pc' = [pc EXCEPT ![self] = Head(stack[self]).pc]
           /\ stack' = [stack EXCEPT ![self] = Tail(stack[self])]
           /\ x' = x

First(self) == F(self) \/ R(self)

L == /\ pc[1] = "L"
     /\ stack' = [stack EXCEPT ![1] = << [ procedure |-> "First", pc |-> "E" ] >> \o stack[1]]
     /\ pc' = [pc EXCEPT ![1] = "F"]
     /\ x' = x

E == /\ pc[1] = "E"
     /\ TRUE
     /\ pc' = [pc EXCEPT ![1] = "Done"]
     /\ UNCHANGED << x, stack >>

Worker == L \/ E

Terminating == /\ \A self \in ProcSet: pc[self] = "Done"
               /\ UNCHANGED vars

Next == Worker \/ (\E self \in ProcSet: First(self)) \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(\A self \in ProcSet: pc[self] = "Done")


Good == \A self \in ProcSet: pc[self] # "E"
====
