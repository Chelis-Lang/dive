---- MODULE ProcedureCrc_C_two_locals_chain ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)
variables t=a+1,u=t+1;
{
P:x:=u; R:return;
};
{
L:call Put(2); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "d0e2018a" /\ chksum(tla) = "46cb6816")
VARIABLES pc, x, stack, a, t, u

vars == << pc, x, stack, a, t, u >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ t = a+1
        /\ u = t+1
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = u
     /\ pc' = "R"
     /\ UNCHANGED << stack, a, t, u >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ t' = Head(stack).t
     /\ u' = Head(stack).u
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "E",
                         t         |->  t,
                         u         |->  u,
                         a         |->  a ] >>
                     \o stack
     /\ t' = a'+1
     /\ u' = t'+1
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, t, u >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
