---- MODULE ProcedureGraph_C_local_restore ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable c=9;
{
F:c:=a+1; call Second(c); R:x:=c; return;
};
procedure Second(b=1)
variable d=4;
{
S:d:=b+1; x:=d; T:return;
};
{
L: call First(2); E: skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "fefce08a" /\ chksum(tla) = "783475e5")
VARIABLES pc, x, stack, a, c, b, d

vars == << pc, x, stack, a, c, b, d >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ c = 9
        (* Procedure Second *)
        /\ b = 1
        /\ d = 4
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ c' = a+1
     /\ /\ b' = c'
        /\ stack' = << [ procedure |->  "Second",
                         pc        |->  "R",
                         d         |->  d,
                         b         |->  b ] >>
                     \o stack
     /\ d' = 4
     /\ pc' = "S"
     /\ UNCHANGED << x, a >>

R == /\ pc = "R"
     /\ x' = c
     /\ pc' = Head(stack).pc
     /\ c' = Head(stack).c
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << b, d >>

First == F \/ R

S == /\ pc = "S"
     /\ d' = b+1
     /\ x' = d'
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, c, b >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ d' = Head(stack).d
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, c >>

Second == S \/ T

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         c         |->  c,
                         a         |->  a ] >>
                     \o stack
     /\ c' = 9
     /\ pc' = "F"
     /\ UNCHANGED << x, b, d >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, c, b, d >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
