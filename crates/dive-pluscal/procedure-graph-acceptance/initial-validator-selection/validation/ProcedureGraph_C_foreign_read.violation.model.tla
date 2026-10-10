---- MODULE ProcedureGraph_C_foreign_read ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:x:=d; return;
};
procedure Second(b=1)
variable d=4;
{
S:x:=b; T:return;
};
{
L: call First(2); E: skip;
}
} *)

VARIABLES pc, x, stack, a, b, d

vars == << pc, x, stack, a, b, d >>

Init == /\ x = 0
        /\ a = 0
        /\ b = 1
        /\ d = 4
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = a
        /\ stack' = << [ procedure |-> "Second", pc |-> "R", d |-> d, b |-> b ] >> \o stack
     /\ d' = 4
     /\ pc' = "S"
     /\ UNCHANGED << x, a >>

R == /\ pc = "R"
     /\ x' = d
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << b, d >>

First == F \/ R

S == /\ pc = "S"
     /\ x' = b
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, b, d >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ d' = Head(stack).d
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a >>

Second == S \/ T

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |-> "First", pc |-> "E", a |-> a ] >> \o stack
     /\ pc' = "F"
     /\ UNCHANGED << x, b, d >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b, d >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")


Good == pc # "E"
====
