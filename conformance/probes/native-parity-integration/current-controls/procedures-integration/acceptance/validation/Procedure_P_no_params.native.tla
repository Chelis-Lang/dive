---- MODULE Procedure_P_no_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put()

begin
P: x:=1; R: return;
end procedure;
begin
L: call Put(); E: skip;
end algorithm; *)

VARIABLES pc, x, stack

vars == << pc, x, stack >>

Init == /\ x = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = 1
     /\ pc' = "R"
     /\ stack' = stack

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

L == /\ pc = "L"
     /\ stack' = << [ procedure |-> "Put", pc |-> "E" ] >> \o stack
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
