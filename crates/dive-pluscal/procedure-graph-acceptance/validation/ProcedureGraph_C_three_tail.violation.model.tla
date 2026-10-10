---- MODULE ProcedureGraph_C_three_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); return;
};
procedure Second(b=1)

{
S:call Third(b); return;
};
procedure Third(c=0)

{
U:x:=c; V:return;
};
{
L: call First(2); E: skip;
}
} *)

VARIABLES pc, x, stack, a, b, c

vars == << pc, x, stack, a, b, c >>

Init == /\ x = 0
        /\ a = 0
        /\ b = 1
        /\ c = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = a
        /\ stack' = << [ procedure |-> "Second", pc |-> Head(stack).pc, b |-> b ] >> \o Tail(stack)
     /\ pc' = "S"
     /\ UNCHANGED << x, a, c >>

First == F

S == /\ pc = "S"
     /\ /\ c' = b
        /\ stack' = << [ procedure |-> "Third", pc |-> Head(stack).pc, c |-> c ] >> \o Tail(stack)
     /\ pc' = "U"
     /\ UNCHANGED << x, a, b >>

Second == S

U == /\ pc = "U"
     /\ x' = c
     /\ pc' = "V"
     /\ UNCHANGED << stack, a, b, c >>

V == /\ pc = "V"
     /\ pc' = Head(stack).pc
     /\ c' = Head(stack).c
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, b >>

Third == U \/ V

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |-> "First", pc |-> "E", a |-> a ] >> \o stack
     /\ pc' = "F"
     /\ UNCHANGED << x, b, c >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b, c >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ Third \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")


Good == pc # "E"
====
