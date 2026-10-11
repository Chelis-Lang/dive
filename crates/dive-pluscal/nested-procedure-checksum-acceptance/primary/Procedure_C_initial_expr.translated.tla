---- MODULE Procedure_C_initial_expr ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=4)
variable b=a+1;
{
P: x:=b; return;
};
{
L: call Put(2); E: skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "c6a2d300" /\ chksum(tla) = "3bbba9f2")
VARIABLES pc, x, stack, a, b

vars == << pc, x, stack, a, b >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 4
        /\ b = a+1
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ x' = b
     /\ pc' = Head(stack).pc
     /\ b' = Head(stack).b
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)

Put == P

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "E",
                         b         |->  b,
                         a         |->  a ] >>
                     \o stack
     /\ b' = a'+1
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a, b >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
