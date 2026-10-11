---- MODULE Procedure_P_param_read_main ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P: x:=a; return;
end procedure;
begin
L: call Put(2); E: x:=a;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "dfca8c15" /\ chksum(tla) = "5db0dcd1")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = a
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
     /\ x' = a
     /\ pc' = "Done"
     /\ UNCHANGED << stack, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
