---- MODULE Recursive_C_self_unbounded ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call First(a+1); R:return;
};
{
L:call First(2); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "14d3c137" /\ chksum(tla) = "9360b2cf")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ a' = a+1
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "R",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "F"
     /\ x' = x

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

First == F \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
