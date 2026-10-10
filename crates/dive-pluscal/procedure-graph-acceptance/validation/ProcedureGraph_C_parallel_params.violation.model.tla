---- MODULE ProcedureGraph_C_parallel_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=1)

{
F:a:=c || c:=a; R:call Second(a,c); Z:return;
};
procedure Second(b=1,d=2)

{
S:b:=d || d:=b; T:x:=b+d; return;
};
{
L:call First(2,3); E:skip;
}
} *)

VARIABLES pc, x, stack, a, c, b, d

vars == << pc, x, stack, a, c, b, d >>

Init == /\ x = 0
        /\ a = 0
        /\ c = 1
        /\ b = 1
        /\ d = 2
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ a' = c
        /\ c' = a
     /\ pc' = "R"
     /\ UNCHANGED << x, stack, b, d >>

R == /\ pc = "R"
     /\ /\ b' = a
        /\ d' = c
        /\ stack' = << [ procedure |-> "Second", pc |-> "Z", b |-> b, d |-> d ] >> \o stack
     /\ pc' = "S"
     /\ UNCHANGED << x, a, c >>

Z == /\ pc = "Z"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ c' = Head(stack).c
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b, d >>

First == F \/ R \/ Z

S == /\ pc = "S"
     /\ /\ b' = d
        /\ d' = b
     /\ pc' = "T"
     /\ UNCHANGED << x, stack, a, c >>

T == /\ pc = "T"
     /\ x' = b + d
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ d' = Head(stack).d
     /\ stack' = Tail(stack)
     /\ UNCHANGED << a, c >>

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
