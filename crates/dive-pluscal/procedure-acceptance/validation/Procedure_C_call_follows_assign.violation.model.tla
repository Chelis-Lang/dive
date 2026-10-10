---- MODULE Procedure_C_call_follows_assign ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: x:=a; return;
};
{
L: x:=1; call Put(x+1); E: skip;
}
} *)

VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == /\ x = 0
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = a
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)

Put == P

L == /\ pc = "L"
     /\ x' = 1
     /\ /\ a' = x' + 1
        /\ stack' = << [ procedure |-> "Put", pc |-> "E", a |-> a ] >> \o stack
     /\ pc' = "P"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")


Good == pc # "E"
====
