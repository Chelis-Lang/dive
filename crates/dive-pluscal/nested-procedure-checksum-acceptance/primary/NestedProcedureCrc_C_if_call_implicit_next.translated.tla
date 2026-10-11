---- MODULE NestedProcedureCrc_C_if_call_implicit_next ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A { variable x=0; procedure Put(a=0) { P:x:=a; R:return; }; { L:if (x=0) { call Put(2); } else { call Put(3); }; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "45aac90e" /\ chksum(tla) = "fd73e53b")
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
           THEN /\ /\ a' = 2
                   /\ stack' = << [ procedure |->  "Put",
                                    pc        |->  "E",
                                    a         |->  a ] >>
                                \o stack
                /\ pc' = "P"
           ELSE /\ /\ a' = 3
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
