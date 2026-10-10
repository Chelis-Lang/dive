---- MODULE Recursive_P_mutual_tail_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable u=a+1;
begin
F:if a>0 then call Second(a-1); return; else x:=u; end if; R:return;
end procedure;
procedure Second(b=0)
variable v=b+2;
begin
S:if b>0 then call First(b-1); return; else x:=v; end if; T:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)

VARIABLES pc, x, stack, a, u, b, v

vars == << pc, x, stack, a, u, b, v >>

Init == /\ x = 0
        /\ a = 0
        /\ u = a + 1
        /\ b = 0
        /\ v = b + 2
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ IF a > 0
           THEN /\ /\ b' = a - 1
                   /\ stack' = << [ procedure |-> "Second", pc |-> Head(stack).pc, v |-> v, b |-> b ] >> \o Tail(stack)
                   /\ u' = Head(stack).u
                /\ v' = b' + 2
                /\ pc' = "S"
                /\ x' = x
           ELSE /\ x' = u
                /\ pc' = "R"
                /\ UNCHANGED << stack, u, b, v >>
     /\ a' = a

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ u' = Head(stack).u
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b, v >>

First == F \/ R

S == /\ pc = "S"
     /\ IF b > 0
           THEN /\ /\ a' = b - 1
                   /\ stack' = << [ procedure |-> "First", pc |-> Head(stack).pc, u |-> u, a |-> a ] >> \o Tail(stack)
                   /\ v' = Head(stack).v
                /\ u' = a' + 1
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = v
                /\ pc' = "T"
                /\ UNCHANGED << stack, a, u, v >>
     /\ b' = b

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ v' = Head(stack).v
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, u >>

Second == S \/ T

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |-> "First", pc |-> "E", u |-> u, a |-> a ] >> \o stack
     /\ u' = a' + 1
     /\ pc' = "F"
     /\ UNCHANGED << x, b, v >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, u, b, v >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
