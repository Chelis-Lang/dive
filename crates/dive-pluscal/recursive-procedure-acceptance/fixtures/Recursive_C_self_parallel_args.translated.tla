---- MODULE Recursive_C_self_parallel_args ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0,c=1)

{
F:if(a>0){ call First(a-1,a); }else{ x:=c; }; R:return;
};
{
L:call First(2,3); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "5c0cf633" /\ chksum(tla) = "32b02e1f")
VARIABLES pc, x, stack, a, c

vars == << pc, x, stack, a, c >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ c = 1
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ IF a>0
           THEN /\ /\ a' = a-1
                   /\ c' = a
                   /\ stack' = << [ procedure |->  "First",
                                    pc        |->  "R",
                                    a         |->  a,
                                    c         |->  c ] >>
                                \o stack
                /\ pc' = "F"
                /\ x' = x
           ELSE /\ x' = c
                /\ pc' = "R"
                /\ UNCHANGED << stack, a, c >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ c' = Head(stack).c
     /\ stack' = Tail(stack)
     /\ x' = x

First == F \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ c' = 3
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         a         |->  a,
                         c         |->  c ] >>
                     \o stack
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, c >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
