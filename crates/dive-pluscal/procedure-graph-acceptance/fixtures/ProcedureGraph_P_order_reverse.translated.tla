---- MODULE ProcedureGraph_P_order_reverse ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Second(b=1)

begin
S: x:=b; T: return;
end procedure;
procedure First(a=0)

begin
F: x:=a; R: return;
end procedure;
begin
L: call First(2); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "6c8ca604" /\ chksum(tla) = "d58c94ee")
VARIABLES pc, x, stack, b, a

vars == << pc, x, stack, b, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Second *)
        /\ b = 1
        (* Procedure First *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

S == /\ pc = "S"
     /\ x' = b
     /\ pc' = "T"
     /\ UNCHANGED << stack, b, a >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a >>

Second == S \/ T

F == /\ pc = "F"
     /\ x' = a
     /\ pc' = "R"
     /\ UNCHANGED << stack, b, a >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, b >>

First == F \/ R

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
     /\ UNCHANGED << x, stack, b, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Second \/ First \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
