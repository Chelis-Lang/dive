---- MODULE Recursive_P_self_local_mutate ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=0;
begin
F:u:=a+1; C:if a>0 then call First(a-1); else x:=u; end if; R:x:=x+u; return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)

VARIABLES pc, x, stack, a, u

vars == << pc, x, stack, a, u >>

Init == /\ x = 0
        /\ a = 0
        /\ u = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ u' = a + 1
     /\ pc' = "C"
     /\ UNCHANGED << x, stack, a >>

C == /\ pc = "C"
     /\ IF a > 0
           THEN /\ /\ a' = a - 1
                   /\ stack' = << [ procedure |-> "First", pc |-> "R", u |-> u, a |-> a ] >> \o stack
                /\ u' = 0
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = u
                /\ pc' = "R"
                /\ UNCHANGED << stack, a, u >>

R == /\ pc = "R"
     /\ x' = x + u
     /\ pc' = Head(stack).pc
     /\ u' = Head(stack).u
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)

First == F \/ C \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |-> "First", pc |-> "E", u |-> u, a |-> a ] >> \o stack
     /\ u' = 0
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, u >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
