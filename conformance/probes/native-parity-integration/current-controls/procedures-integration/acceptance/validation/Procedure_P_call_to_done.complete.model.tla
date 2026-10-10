---- MODULE Procedure_P_call_to_done ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: x:=a; R: return;
end procedure;
begin
L: call Put(2);
end algorithm; *)

VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == /\ x = 0
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = a
     /\ pc' = "R"
     /\ UNCHANGED << stack, a >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |-> "Put", pc |-> "Done", a |-> a ] >> \o stack
     /\ pc' = "P"
     /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
