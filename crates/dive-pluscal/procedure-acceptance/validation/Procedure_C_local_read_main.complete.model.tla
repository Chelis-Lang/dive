---- MODULE Procedure_C_local_read_main ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variable b=1;
{
P: x:=a; return;
};
{
L: call Put(2); E: x:=b;
}
} *)

VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

Init == /\ x = 0
        /\ a = 0
        /\ b = 1
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = a
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)

Put == P

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |-> "Put", pc |-> "E", b |-> b, a |-> a ] >> \o stack
     /\ b' = 1
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ x' = b
     /\ pc' = "Done"
     /\ UNCHANGED << stack, a, b >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
