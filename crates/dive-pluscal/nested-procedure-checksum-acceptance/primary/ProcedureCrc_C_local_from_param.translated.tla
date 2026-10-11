---- MODULE ProcedureCrc_C_local_from_param ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variable t=a+1;
{
P:x:=t; R:return;
};
{
L:call Put(2); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "83b7137c" /\ chksum(tla) = "f1b1a92b")
VARIABLES pc, x, stack, a, t

vars == << pc, x, stack, a, t >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ t = a+1
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = t
     /\ pc' = "R"
     /\ UNCHANGED << stack, a, t >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ t' = Head(stack).t
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "E",
                         t         |->  t,
                         a         |->  a ] >>
                     \o stack
     /\ t' = a'+1
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
