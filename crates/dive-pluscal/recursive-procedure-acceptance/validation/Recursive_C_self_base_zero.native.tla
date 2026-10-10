---- MODULE Recursive_C_self_base_zero ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:if(a>0){ call First(a-1); }else{ x:=x+1; }; R:x:=x+1; return;
};
{
L:call First(0); E:skip;
}
} *)

VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == /\ x = 0
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ IF a > 0
           THEN /\ /\ a' = a - 1
                   /\ stack' = << [ procedure |-> "First", pc |-> "R", a |-> a ] >> \o stack
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = x + 1
                /\ pc' = "R"
                /\ UNCHANGED << stack, a >>

R == /\ pc = "R"
     /\ x' = x + 1
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)

First == F \/ R

L == /\ pc = "L"
     /\ /\ a' = 0
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
