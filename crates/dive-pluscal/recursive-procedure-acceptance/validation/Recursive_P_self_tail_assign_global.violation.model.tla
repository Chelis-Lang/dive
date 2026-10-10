---- MODULE Recursive_P_self_tail_assign_global ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:x:=a; if a>0 then call First(a-1); return; else skip; end if; R:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)

VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == /\ x = 0
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ x' = a
     /\ IF a > 0
           THEN /\ a' = a - 1
                /\ pc' = "F"
           ELSE /\ TRUE
                /\ pc' = "R"
                /\ a' = a
     /\ stack' = stack

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

First == F \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |-> "First", pc |-> "E", a |-> a ] >> \o stack
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")


Good == pc # "E"
====
