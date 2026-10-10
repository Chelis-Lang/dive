---- MODULE Recursive_P_duplicate_scope ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure First(a=0)

begin
F:call Second(a); R:return;
end procedure;
procedure Second(a=1)

begin
S:call First(a); T:return;
end procedure;
begin
L:call First(2); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a667c38e" /\ chksum(tla) = "409f9b8d")
\* Parameter a of procedure First at line 5 col 17 changed to a_
VARIABLES pc, x, stack, a_, a

vars == << pc, x, stack, a_, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure First *)
        /\ a_ = 0
        (* Procedure Second *)
        /\ a = 1
        /\ stack = << >>
        /\ pc = "L"

F == /\ pc = "F"
     /\ /\ a' = a_
        /\ stack' = << [ procedure |->  "Second",
                         pc        |->  "R",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "S"
     /\ UNCHANGED << x, a_ >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a_' = Head(stack).a_
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a >>

First == F \/ R

S == /\ pc = "S"
     /\ /\ a_' = a
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "T",
                         a_        |->  a_ ] >>
                     \o stack
     /\ pc' = "F"
     /\ UNCHANGED << x, a >>

T == /\ pc = "T"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ UNCHANGED << x, a_ >>

Second == S \/ T

L == /\ pc = "L"
     /\ /\ a_' = 2
        /\ stack' = << [ procedure |->  "First",
                         pc        |->  "E",
                         a_        |->  a_ ] >>
                     \o stack
     /\ pc' = "F"
     /\ UNCHANGED << x, a >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a_, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == First \/ Second \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
