---- MODULE ProcedureGraph_C_read_stack ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:x:=Len(stack); return;
};
procedure Second(b=1)

{
S:x:=Len(stack); T:return;
};
{
L: call First(2); E: skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "fa838c1a" /\ chksum(tla) = "f7c07fd5")
VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        (* Procedure Second *)
        /\ b = 1
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = a
        /\ stack' = << [ procedure |->  "Second",
                         pc        |->  "R",
                         b         |->  b ] >>
                     \o stack
     /\ pc' = "S"
     /\ UNCHANGED << x, a >>

R == /\ pc = "R"
     /\ x' = Len(stack)
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ b' = b

First == F \/ R

S == /\ pc = "S"
     /\ x' = Len(stack)
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, b >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a >>

Second == S \/ T

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
