---- MODULE Recursive_P_self_with ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:with n=a-1 do if a>0 then call First(n); else x:=x+1; end if; end with; R:return;
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
     /\ LET n == a - 1 IN
           IF a > 0
              THEN /\ /\ a' = n
                      /\ stack' = << [ procedure |-> "First", pc |-> "R", a |-> a ] >> \o stack
                   /\ pc' = "F"
                   /\ x' = x
              ELSE /\ x' = x + 1
                   /\ pc' = "R"
                   /\ UNCHANGED << stack, a >>

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
