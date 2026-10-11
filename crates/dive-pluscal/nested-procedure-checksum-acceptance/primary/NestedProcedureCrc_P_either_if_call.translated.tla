---- MODULE NestedProcedureCrc_P_either_if_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:either if x=0 then call Put(2); goto E; else skip; end if; or skip; end either; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "ea405aa4" /\ chksum(tla) = "af9fa3d2")
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
     /\ \/ /\ IF x=0
                 THEN /\ /\ a' = 2
                         /\ stack' = << [ procedure |->  "Put",
                                          pc        |->  "E",
                                          a         |->  a ] >>
                                      \o stack
                      /\ pc' = "P"
                 ELSE /\ TRUE
                      /\ pc' = "E"
                      /\ UNCHANGED << stack, a >>
        \/ /\ TRUE
           /\ pc' = "E"
           /\ UNCHANGED <<stack, a>>
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
