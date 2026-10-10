---- MODULE ProcedureGraph_P_reserved_parameter ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F: x:=a; R: return;
end procedure;
procedure Second(Next=1)

begin
S:x:=Next; T:return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "1915c89a" /\ chksum(tla) = "91116316")
VARIABLES pc, x, stack, a, Next

vars == << pc, x, stack, a, Next >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a = 0
        (* Procedure Second *)
        /\ Next = 1
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ x' = a
     /\ pc' = "R"
     /\ UNCHANGED << stack, a, Next >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, Next >>

First == F \/ R

S == /\ pc = "S"
     /\ x' = Next
     /\ pc' = "T"
     /\ UNCHANGED << stack, a, Next >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ Next' = Head(stack).Next
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
     /\ UNCHANGED << x, Next >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, Next >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
