---- MODULE Procedure_P_either ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: either x:=a; or x:=1; end either; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "3fe1871d" /\ chksum(tla) = "6a183214")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ \/ /\ x' = a
        \/ /\ x' = 1
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)

Put == P

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "E",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
