---- MODULE ProcedureCrc_C_return_only ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P:return;
};
{
L:call Put(2); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "4f0bb3f0" /\ chksum(tla) = "585f6393")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

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
