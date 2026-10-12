---- MODULE ProcedureGraph_C_three_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure First(a=0)

{
F:call Second(a); R:return;
};
procedure Second(b=1)

{
S:call Third(b+1); T:x:=x+b; return;
};
procedure Third(c=0)
variable d=c+1;
{
U:x:=d; V:return;
};
{
L: call First(2); E: skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "7f484c51" /\ chksum(tla) = "4956b08a")
VARIABLES pc, x, stack, a, b, c, d

vars == << pc, x, stack, a, b, c, d >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        (* Procedure Second *)
        /\ b = 1
        (* Procedure Third *)
        /\ c = 0
        /\ d = c+1
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = a
        /\ stack' = << [ procedure |->  "Second",
                         pc        |->  "R",
                         b         |->  b ] >>
                     \o stack
     /\ pc' = "S"
     /\ UNCHANGED << x, a, c, d >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b, c, d >>

First == F \/ R

S == /\ pc = "S"
     /\ /\ c' = b+1
        /\ stack' = << [ procedure |->  "Third",
                         pc        |->  "T",
                         d         |->  d,
                         c         |->  c ] >>
                     \o stack
     /\ d' = c'+1
     /\ pc' = "U"
     /\ UNCHANGED << x, a, b >>

T == /\ pc = "T"
     /\ x' = x+b
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << a, c, d >>

Second == S \/ T

U == /\ pc = "U"
     /\ x' = d
     /\ pc' = "V"
     /\ UNCHANGED << stack, a, b, c, d >>

V == /\ pc = "V"
     /\ pc' = Head(stack).pc
     /\ d' = Head(stack).d
     /\ c' = Head(stack).c
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, b >>

Third == U \/ V

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "F"
     /\ UNCHANGED << x, b, c, d >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b, c, d >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ Third \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
