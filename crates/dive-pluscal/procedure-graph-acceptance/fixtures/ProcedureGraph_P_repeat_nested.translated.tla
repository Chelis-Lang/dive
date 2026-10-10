---- MODULE ProcedureGraph_P_repeat_nested ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:call Second(a+1); Z:return;
end procedure;
procedure Second(b=1)

begin
S: x:=b; T: return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "838a9caf" /\ chksum(tla) = "40052012")
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
     /\ /\ b' = a+1
        /\ stack' = << [ procedure |->  "Second",
                         pc        |->  "Z",
                         b         |->  b ] >>
                     \o stack
     /\ pc' = "S"
     /\ UNCHANGED << x, a >>

Z == /\ pc = "Z"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b >>

First == F \/ R \/ Z

S == /\ pc = "S"
     /\ x' = b
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
