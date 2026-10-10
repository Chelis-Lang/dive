---- MODULE Recursive_C_mutual_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+1;
{
F:if(a>0){ call Second(a-1); }else{ x:=u; }; R:x:=x+u; return;
};
procedure Second(b=0)
variable v=b+2;
{
S:if(b>0){ call First(b-1); }else{ x:=v; }; T:x:=x+v; return;
};
{
L:call First(2); E:skip;
}
} *)

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
                   /\ stack' = << [ procedure |-> "Second", pc |-> "R", v |-> v, b |-> b ] >> \o stack
                /\ v' = b' + 2
                /\ pc' = "S"
                /\ x' = x
           ELSE /\ x' = u
                /\ pc' = "R"
                /\ UNCHANGED << stack, b, v >>
     /\ UNCHANGED << a, u >>

R == /\ pc = "R"
     /\ x' = x + u
     /\ pc' = Head(stack).pc
     /\ u' = Head(stack).u
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << b, v >>

First == F \/ R

S == /\ pc = "S"
     /\ IF b > 0
           THEN /\ /\ a' = b - 1
                   /\ stack' = << [ procedure |-> "First", pc |-> "T", u |-> u, a |-> a ] >> \o stack
                /\ u' = a' + 1
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = v
                /\ pc' = "T"
                /\ UNCHANGED << stack, a, u >>
     /\ UNCHANGED << b, v >>

T == /\ pc = "T"
     /\ x' = x + v
     /\ pc' = Head(stack).pc
     /\ v' = Head(stack).v
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << a, u >>

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


Good == pc # "E"
====
