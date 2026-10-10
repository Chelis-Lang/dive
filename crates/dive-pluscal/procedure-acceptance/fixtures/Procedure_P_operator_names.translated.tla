---- MODULE Procedure_P_operator_names ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(Next=0)

begin
P: x:=Next; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "61e7d7ee" /\ chksum(tla) = "69a5777")
VARIABLES pc, x, stack, Next

vars == << pc, x, stack, Next >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ Next = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = Next
     /\ pc' = Head(stack).pc
     /\ Next' = Head(stack).Next
     /\ stack' = Tail(stack)

Put == P

L == /\ pc = "L"
     /\ /\ Next' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "E",
                         Next      |->  Next ] >>
                     \o stack
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, Next >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
