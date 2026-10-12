---- MODULE ProcedureGraph_P_two_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0,c=1)

begin
F:call Second(a,c); R:return;
end procedure;
procedure Second(b=1,d=2)

begin
S:x:=b+d; T:return;
end procedure;
begin
L:call First(2,3); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "583e2ada" /\ chksum(tla) = "9bc7c9f3")
VARIABLES pc, x, stack, a, c, b, d

vars == << pc, x, stack, a, c, b, d >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ c = 1
        (* Procedure Second *)
        /\ b = 1
        /\ d = 2
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = a
        /\ d' = c
        /\ stack' = << [ procedure |->  "Second",
                         pc        |->  "R",
                         b         |->  b,
                         d         |->  d ] >>
                     \o stack
     /\ pc' = "S"
     /\ UNCHANGED << x, a, c >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ c' = Head(stack).c
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b, d >>

First == F \/ R

S == /\ pc = "S"
     /\ x' = b+d
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, c, b, d >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ d' = Head(stack).d
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a, c >>

Second == S \/ T

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ c' = 3
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         a         |->  a,
                         c         |->  c ] >>
                     \o stack
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
