---- MODULE Procedure_P_proc_label_collision ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
Put: x:=a; return;
end procedure;
begin
L: call Put(2); E: skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "8a6b27a0" /\ chksum(tla) = "2593a843")
\* Label Put of procedure Put at line 8 col 6 changed to Put_
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

Put_ == /\ pc = "Put_"
        /\ x' = a
        /\ pc' = Head(stack).pc
        /\ a' = Head(stack).a
        /\ stack' = Tail(stack)

Put == Put_

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "E",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "Put_"
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
