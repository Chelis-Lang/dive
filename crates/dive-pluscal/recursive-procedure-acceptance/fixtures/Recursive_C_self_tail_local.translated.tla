---- MODULE Recursive_C_self_tail_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable u=a+10;
{
F:if(a>0){ call First(a-1); return; }else{ x:=u; }; R:return;
};
{
L:call First(2); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "c26445b5" /\ chksum(tla) = "2e5c0ad2")
VARIABLES pc, x, stack, a, u

vars == << pc, x, stack, a, u >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ u = a+10
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ IF a>0
           THEN /\ a' = a-1
                /\ u' = a'+10
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = u
                /\ pc' = "R"
                /\ UNCHANGED << a, u >>
     /\ stack' = stack

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ u' = Head(stack).u
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

First == F \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         u         |->  u,
                         a         |->  a ] >>
                     \o stack
     /\ u' = a'+10
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, u >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
