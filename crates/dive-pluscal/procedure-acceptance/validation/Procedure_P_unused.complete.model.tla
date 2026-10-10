---- MODULE Procedure_P_unused ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: x:=a; return;
end procedure;
begin
L: skip;
end algorithm; *)

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
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
