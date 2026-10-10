---- MODULE ProcedureGraph_C_restore_order ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable c=2;
{
F:c:=a+1; call Second(a); R:x:=c+a; return;
};
procedure Second(b=1)
variable d=b+1;
{
S:x:=d; T:return;
};
{
L: call First(2); E: skip;
}
} *)

VARIABLES pc, x, stack, a, c, b, d

vars == << pc, x, stack, a, c, b, d >>

Init == /\ x = 0
        /\ a = 0
        /\ c = 2
        /\ b = 1
        /\ d = b + 1
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ c' = a + 1
     /\ /\ b' = a
        /\ stack' = << [ procedure |-> "Second", pc |-> "R", d |-> d, b |-> b ] >> \o stack
     /\ d' = b' + 1
     /\ pc' = "S"
     /\ UNCHANGED << x, a >>

R == /\ pc = "R"
     /\ x' = c + a
     /\ pc' = Head(stack).pc
     /\ c' = Head(stack).c
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << b, d >>

First == F \/ R

S == /\ pc = "S"
     /\ x' = d
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, c, b, d >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ d' = Head(stack).d
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, c >>

Second == S \/ T

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |-> "First", pc |-> "E", c |-> c, a |-> a ] >> \o stack
     /\ c' = 2
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
