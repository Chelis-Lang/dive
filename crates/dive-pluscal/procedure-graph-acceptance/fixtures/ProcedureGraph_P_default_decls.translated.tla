---- MODULE ProcedureGraph_P_default_decls ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a)
variable c;
begin
F:call Second(a); R:return;
end procedure;
procedure Second(b=1)
variable d;
begin
S: x:=b; T: return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "d84daa92" /\ chksum(tla) = "6f77e155")
CONSTANT defaultInitValue
VARIABLES pc, x, stack, a, c, b, d

vars == << pc, x, stack, a, c, b, d >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = defaultInitValue
        /\ c = defaultInitValue
        (* Procedure Second *)
        /\ b = 1
        /\ d = defaultInitValue
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ b' = a
        /\ stack' = << [ procedure |->  "Second",
                         pc        |->  "R",
                         d         |->  d,
                         b         |->  b ] >>
                     \o stack
     /\ d' = defaultInitValue
     /\ pc' = "S"
     /\ UNCHANGED << x, a, c >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ c' = Head(stack).c
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b, d >>

First == F \/ R

S == /\ pc = "S"
     /\ x' = b
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
     /\ c' = defaultInitValue
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
