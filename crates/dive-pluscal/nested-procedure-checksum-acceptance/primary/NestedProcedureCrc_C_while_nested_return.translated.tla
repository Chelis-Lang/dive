---- MODULE NestedProcedureCrc_C_while_nested_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:while (a=0) { if (a=0) { return; } else { skip; }; }; R:return; }; { L:call Put(2); E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "2e3c0493" /\ chksum(tla) = "717e0d3d")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ IF a=0
           THEN /\ IF a=0
                      THEN /\ pc' = Head(stack).pc
                           /\ a' = Head(stack).a
                           /\ stack' = Tail(stack)
                      ELSE /\ TRUE
                           /\ pc' = "P"
                           /\ UNCHANGED << stack, a >>
           ELSE /\ pc' = "R"
                /\ UNCHANGED << stack, a >>
     /\ x' = x

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

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
