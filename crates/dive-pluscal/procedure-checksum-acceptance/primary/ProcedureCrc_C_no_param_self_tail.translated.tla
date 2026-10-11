---- MODULE ProcedureCrc_C_no_param_self_tail ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put()

{
P:call Put(); return;
};
{
L:call Put(); E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "6b91ec5c" /\ chksum(tla) = "99a21bbf")
VARIABLES pc, x, stack

vars == << pc, x, stack >>

Init == (* Global variables *)
        /\ x = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ 
     /\ pc' = "P"
     /\ UNCHANGED << x, stack >>

Put == P

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
