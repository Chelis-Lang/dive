---- MODULE Recursive_C_mutual_tail_unbounded ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a+1); return;
};
procedure Second(b=0)

{
S:call First(b+1); return;
};
{
L:call First(2); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "f6f1d5f7" /\ chksum(tla) = "775d3f81")
VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        (* Procedure Second *)
        /\ b = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = a+1
        /\ stack' = << [ procedure |->  "Second",
                         pc        |->  Head(stack).pc,
                         b         |->  b ] >>
                     \o Tail(stack)
     /\ pc' = "S"
     /\ UNCHANGED << x, a >>

First == F

S == /\ pc = "S"
     /\ /\ a' = b+1
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  Head(stack).pc,
                         a         |->  a ] >>
                     \o Tail(stack)
     /\ pc' = "F"
     /\ UNCHANGED << x, b >>

Second == S

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "F"
     /\ UNCHANGED << x, b >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
