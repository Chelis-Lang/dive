---- MODULE ProcedureCrc_P_zero_slots ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put()

begin
P:x:=2; R:return;
end procedure;
begin
L:call Put(); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "5563ddf3" /\ chksum(tla) = "731e407")
VARIABLES pc, x, stack

vars == << pc, x, stack >>

Init == (* Global variables *)
        /\ x = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = 2
     /\ pc' = "R"
     /\ stack' = stack

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

L == /\ pc = "L"
     /\ stack' = << [ procedure |->  "Put",
                      pc        |->  "E" ] >>
                  \o stack
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
