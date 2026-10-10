---- MODULE ProcedureGraph_P_call_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); return;
end procedure;
procedure Second(b=1)

begin
S: x:=b; T: return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)

VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

Init == /\ x = 0
        /\ a = 0
        /\ b = 1
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = a
        /\ stack' = << [ procedure |-> "Second", pc |-> Head(stack).pc, b |-> b ] >> \o Tail(stack)
     /\ pc' = "S"
     /\ UNCHANGED << x, a >>

First == F

S == /\ pc = "S"
     /\ x' = b
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, b >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a >>

Second == S \/ T

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |-> "First", pc |-> "E", a |-> a ] >> \o stack
     /\ pc' = "F"
     /\ UNCHANGED << x, b >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")


Good == pc # "E"
====
