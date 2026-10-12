---- MODULE ProcedureGraph_P_separate_local_init ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:return;
end procedure;
procedure Second(b=1)
variable d=b+1,e=d+1;
begin
S:x:=d+e; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "1b1eb608" /\ chksum(tla) = "ea21e102")
VARIABLES pc, x, stack, a, b, d, e

vars == << pc, x, stack, a, b, d, e >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        (* Procedure Second *)
        /\ b = 1
        /\ d = b+1
        /\ e = d+1
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = a
        /\ stack' = << [ procedure |->  "Second",
                         pc        |->  "R",
                         d         |->  d,
                         e         |->  e,
                         b         |->  b ] >>
                     \o stack
     /\ d' = b'+1
     /\ e' = d'+1
     /\ pc' = "S"
     /\ UNCHANGED << x, a >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b, d, e >>

First == F \/ R

S == /\ pc = "S"
     /\ x' = d+e
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, b, d, e >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ d' = Head(stack).d
     /\ e' = Head(stack).e
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
     /\ UNCHANGED << x, b, d, e >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b, d, e >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
