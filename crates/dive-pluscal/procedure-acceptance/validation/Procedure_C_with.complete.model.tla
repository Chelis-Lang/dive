---- MODULE Procedure_C_with ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P: with(b=a+1){ x:=b; }; return;
};
{
L: call Put(2); E: skip;
}
} *)

VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == /\ x = 0
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ LET b == a + 1 IN
           x' = b
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)

Put == P

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |-> "Put", pc |-> "E", a |-> a ] >> \o stack
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
