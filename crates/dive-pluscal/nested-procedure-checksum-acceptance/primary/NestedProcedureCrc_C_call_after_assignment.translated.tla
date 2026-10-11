---- MODULE NestedProcedureCrc_C_call_after_assignment ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:if (x=0) { x:=1; call Put(2); goto E; } else { skip; }; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "6be3e879" /\ chksum(tla) = "e9cd735a")
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
     /\ IF x=0
           THEN /\ x' = 1
                /\ /\ a' = 2
                   /\ stack' = << [ procedure |->  "Put",
                                    pc        |->  "E",
                                    a         |->  a ] >>
                                \o stack
                /\ pc' = "P"
           ELSE /\ TRUE
                /\ pc' = "E"
                /\ UNCHANGED << x, stack, a >>

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
