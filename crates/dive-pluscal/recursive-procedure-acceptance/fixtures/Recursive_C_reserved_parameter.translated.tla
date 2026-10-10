---- MODULE Recursive_C_reserved_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(Next=0)

{
F:call First(Next); R:return;
};
{
L:call First(2); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "ef5e84" /\ chksum(tla) = "90c48a99")
VARIABLES pc, x, stack, Next

vars == << pc, x, stack, Next >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ Next = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ Next' = Next
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "R",
                         Next      |->  Next ] >>
                     \o stack
     /\ pc' = "F"
     /\ x' = x

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ Next' = Head(stack).Next
     /\ stack' = Tail(stack)
     /\ x' = x

First == F \/ R

L == /\ pc = "L"
     /\ /\ Next' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         Next      |->  Next ] >>
                     \o stack
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, Next >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
