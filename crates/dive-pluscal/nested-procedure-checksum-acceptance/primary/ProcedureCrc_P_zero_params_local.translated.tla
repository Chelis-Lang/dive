---- MODULE ProcedureCrc_P_zero_params_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put()
variable t=1;
begin
P:x:=t; R:return;
end procedure;
begin
L:call Put(); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a0ad46a" /\ chksum(tla) = "dd52994f")
VARIABLES pc, x, stack, t

vars == << pc, x, stack, t >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ t = 1
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = t
     /\ pc' = "R"
     /\ UNCHANGED << stack, t >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ t' = Head(stack).t
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

L == /\ pc = "L"
     /\ stack' = << [ procedure |->  "Put",
                      pc        |->  "E",
                      t         |->  t ] >>
                  \o stack
     /\ t' = 1
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, t >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
