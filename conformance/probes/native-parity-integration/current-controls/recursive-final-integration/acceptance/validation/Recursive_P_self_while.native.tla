---- MODULE Recursive_P_self_while ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:while a>0 do call First(a-1); C:a:=a-1; end while; R:x:=x+1; return;
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
     /\ IF a > 0
           THEN /\ /\ a' = a - 1
                   /\ stack' = << [ procedure |-> "First", pc |-> "C", a |-> a ] >> \o stack
                /\ pc' = "F"
           ELSE /\ pc' = "R"
                /\ UNCHANGED << stack, a >>
     /\ x' = x

C == /\ pc = "C"
     /\ a' = a - 1
     /\ pc' = "F"
     /\ UNCHANGED << x, stack >>

R == /\ pc = "R"
     /\ x' = x + 1
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)

First == F \/ C \/ R

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

====
