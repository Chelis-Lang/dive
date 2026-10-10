---- MODULE Recursive_C_self_no_params_unbounded ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{
F:call First(); R:return;
};
{
L:call First(); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "bfa0a186" /\ chksum(tla) = "b773aaeb")
VARIABLES pc, x, stack

vars == << pc, x, stack >>

Init == (* Global variables *)
        /\ x = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ stack' = << [ procedure |->  "First",
                      pc        |->  "R" ] >>
                  \o stack
     /\ pc' = "F"
     /\ x' = x

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ stack' = Tail(stack)
     /\ x' = x

First == F \/ R

L == /\ pc = "L"
     /\ stack' = << [ procedure |->  "First",
                      pc        |->  "E" ] >>
                  \o stack
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
