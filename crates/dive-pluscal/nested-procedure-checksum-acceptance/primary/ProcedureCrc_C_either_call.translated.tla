---- MODULE ProcedureCrc_C_either_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {
variable x=0;
procedure Put(a=0)

{
P:x:=a; R:return;
};
{
L:either {call Put(2); goto E;} or {call Put(3); goto E; }; E:skip;
}
} *)
\* BEGIN TRANSLATION (chksum(pcal) = "2e063572" /\ chksum(tla) = "480c53d7")
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
     /\ \/ /\ /\ a' = 2
              /\ stack' = << [ procedure |->  "Put",
                               pc        |->  "E",
                               a         |->  a ] >>
                           \o stack
           /\ pc' = "P"
        \/ /\ /\ a' = 3
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
