---- MODULE ProcedureCrc_P_repeat_calls ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)

begin
P:x:=a; R:return;
end procedure;
begin
L:call Put(2); M:call Put(x+1); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "b717bf91" /\ chksum(tla) = "3b33d84f")
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
     /\ pc' = "R"
     /\ UNCHANGED << stack, a >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "M",
                         a         |->  a ] >>
                     \o stack
     /\ pc' = "P"
     /\ x' = x

M == /\ pc = "M"
     /\ /\ a' = x+1
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

Next == Put \/ L \/ M \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
