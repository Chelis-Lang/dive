---- MODULE ProcedureGraph_C_parameter_order ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=a+1)

{
F:call Second(a,c); R:return;
};
procedure Second(b=1,d=b+1)

{
S:x:=b+d; T:return;
};
{
L:call First(2,3); E:skip;
}
} *)

VARIABLES pc, x, stack, a, c, b, d

vars == << pc, x, stack, a, c, b, d >>

Init == /\ x = 0
        /\ a = 0
        /\ c = a + 1
        /\ b = 1
        /\ d = b + 1
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = a
        /\ d' = c
        /\ stack' = << [ procedure |-> "Second", pc |-> "R", b |-> b, d |-> d ] >> \o stack
     /\ pc' = "S"
     /\ UNCHANGED << x, a, c >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ c' = Head(stack).c
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b, d >>

First == F \/ R

S == /\ pc = "S"
     /\ x' = b + d
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, c, b, d >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ d' = Head(stack).d
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, c >>

Second == S \/ T

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ c' = 3
        /\ stack' = << [ procedure |-> "First", pc |-> "E", a |-> a, c |-> c ] >> \o stack
     /\ pc' = "F"
     /\ UNCHANGED << x, b, d >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, c, b, d >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")


Good == pc # "E"
====
