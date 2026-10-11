---- MODULE ProcedureCrc_P_default_local ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A
variable x=0;
procedure Put(a=0)
variable t;
begin
P:t:=a; Q:x:=t; R:return;
end procedure;
begin
L:call Put(2); E:skip;
end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "534c3dff" /\ chksum(tla) = "70a63994")
CONSTANT defaultInitValue
VARIABLES pc, x, stack, a, t

vars == << pc, x, stack, a, t >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ t = defaultInitValue
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ t' = a
     /\ pc' = "Q"
     /\ UNCHANGED << x, stack, a >>

Q == /\ pc = "Q"
     /\ x' = t
     /\ pc' = "R"
     /\ UNCHANGED << stack, a, t >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ t' = Head(stack).t
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ Q \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "E",
                         t         |->  t,
                         a         |->  a ] >>
                     \o stack
     /\ t' = defaultInitValue
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, t >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
