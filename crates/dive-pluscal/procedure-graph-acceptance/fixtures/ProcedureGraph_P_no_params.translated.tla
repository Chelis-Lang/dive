---- MODULE ProcedureGraph_P_no_params ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First()

begin
F:call Second(); R:return;
end procedure;
procedure Second()

begin
S:x:=1; T:return;
end procedure;
begin
L:call First(); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "68bac0cd" /\ chksum(tla) = "83ac60d2")
VARIABLES pc, x, stack

vars == << pc, x, stack >>

Init == (* Global variables *)
        /\ x = 0
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ stack' = << [ procedure |->  "Second",
                      pc        |->  "R" ] >>
                  \o stack
     /\ pc' = "S"
     /\ x' = x

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ stack' = Tail(stack)
     /\ x' = x

First == F \/ R

S == /\ pc = "S"
     /\ x' = 1
     /\ pc' = "T"
     /\ stack' = stack

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ stack' = Tail(stack)
     /\ x' = x

Second == S \/ T

L == /\ pc = "L"
     /\ stack' = << [ procedure |->  "First",
                      pc        |->  "E" ] >>
                  \o stack
     /\ pc' = "F"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
