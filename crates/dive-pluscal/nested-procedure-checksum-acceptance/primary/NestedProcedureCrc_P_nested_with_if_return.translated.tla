---- MODULE NestedProcedureCrc_P_nested_with_if_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:with q=a do if q=0 then return; else x:=q; end if; end with; R:return; end procedure; begin L:call Put(2); E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "40a0c720" /\ chksum(tla) = "4a17b065")
VARIABLES pc, x, stack, a

vars == << pc, x, stack, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ LET q == a IN
          IF q=0
             THEN /\ pc' = Head(stack).pc
                  /\ a' = Head(stack).a
                  /\ stack' = Tail(stack)
                  /\ x' = x
             ELSE /\ x' = q
                  /\ pc' = "R"
                  /\ UNCHANGED << stack, a >>

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
