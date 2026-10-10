---- MODULE ProcedureGraph_P_call_return_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)
variable c=a+1;
begin
F:call Second(c); return;
end procedure;
procedure Second(b=1)
variable d=b+2;
begin
S:x:=d; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "456f2a60" /\ chksum(tla) = "5ef8c1d4")
VARIABLES pc, x, stack, a, c, b, d

vars == << pc, x, stack, a, c, b, d >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        /\ c = a+1
        (* Procedure Second *)
        /\ b = 1
        /\ d = b+2
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = c
        /\ c' = Head(stack).c
        /\ stack' = << [ procedure |->  "Second",
                         pc        |->  Head(stack).pc,
                         d         |->  d,
                         b         |->  b ] >>
                     \o Tail(stack)
     /\ d' = b'+2
     /\ pc' = "S"
     /\ UNCHANGED << x, a >>

First == F

S == /\ pc = "S"
     /\ x' = d
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, c, b, d >>

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
     /\ c' = a'+1
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
