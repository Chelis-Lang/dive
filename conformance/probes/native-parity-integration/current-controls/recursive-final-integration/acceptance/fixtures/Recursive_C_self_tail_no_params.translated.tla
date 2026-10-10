---- MODULE Recursive_C_self_tail_no_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First()

{
F:call First(); return;
};
{
L:call First(); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "e9f8242f" /\ chksum(tla) = "5b3120a1")
VARIABLES pc, x, stack

vars == << pc, x, stack >>

Init == (* Global variables *)
        /\ x = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ 
     /\ pc' = "F"
     /\ UNCHANGED << x, stack >>

First == F

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
