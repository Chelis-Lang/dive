---- MODULE ProcedureGraph_C_duplicate_locals ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)
variable c=0;
{
F: x:=a; R: return;
};
procedure Second(b=1)
variable c=1;
{
S: x:=b; T: return;
};
{
L: call First(2); E: skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "478fc78f" /\ chksum(tla) = "2f9b10c5")
\* Procedure variable c of procedure First at line 6 col 10 changed to c_
VARIABLES pc, x, stack, a, c_, b, c

vars == << pc, x, stack, a, c_, b, c >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ c_ = 0
        (* Procedure Second *)
        /\ b = 1
        /\ c = 1
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ x' = a
     /\ pc' = "R"
     /\ UNCHANGED << stack, a, c_, b, c >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ c_' = Head(stack).c_
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b, c >>

First == F \/ R

S == /\ pc = "S"
     /\ x' = b
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, c_, b, c >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ c' = Head(stack).c
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, c_ >>

Second == S \/ T

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         c_        |->  c_,
                         a         |->  a ] >>
                     \o stack
     /\ c_' = 0
     /\ pc' = "F"
     /\ UNCHANGED << x, b, c >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, c_, b, c >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
