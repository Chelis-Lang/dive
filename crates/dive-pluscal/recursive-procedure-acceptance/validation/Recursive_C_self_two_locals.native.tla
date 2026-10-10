---- MODULE Recursive_C_self_two_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+1,v=u+1;
{
F:if(a>0){ call First(a-1); }else{ x:=u+v; }; R:x:=x+u+v; return;
};
{
L:call First(2); E:skip;
}
} *)

VARIABLES pc, x, stack, a, u, v

vars == << pc, x, stack, a, u, v >>

Init == /\ x = 0
        /\ a = 0
        /\ u = a + 1
        /\ v = u + 1
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ IF a > 0
           THEN /\ /\ a' = a - 1
                   /\ stack' = << [ procedure |-> "First", pc |-> "R", u |-> u, v |-> v, a |-> a ] >> \o stack
                /\ u' = a' + 1
                /\ v' = u' + 1
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = u + v
                /\ pc' = "R"
                /\ UNCHANGED << stack, a, u, v >>

R == /\ pc = "R"
     /\ x' = x + u + v
     /\ pc' = Head(stack).pc
     /\ u' = Head(stack).u
     /\ v' = Head(stack).v
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)

First == F \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |-> "First", pc |-> "E", u |-> u, v |-> v, a |-> a ] >> \o stack
     /\ u' = a' + 1
     /\ v' = u' + 1
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, u, v >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
