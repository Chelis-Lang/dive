---- MODULE NestedProcedureCrc_P_while_nested_either_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(a=0) begin P:x:=a; R:return; end procedure; begin L:while x=0 do either call Put(2); goto B; or skip; end either; B:skip; end while; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "92484cd" /\ chksum(tla) = "3d406577")
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
           THEN /\ \/ /\ /\ a' = 2
                         /\ stack' = << [ procedure |->  "Put",
                                          pc        |->  "B",
                                          a         |->  a ] >>
                                      \o stack
                      /\ pc' = "P"
                   \/ /\ TRUE
                      /\ pc' = "B"
                      /\ UNCHANGED <<stack, a>>
           ELSE /\ pc' = "E"
                /\ UNCHANGED << stack, a >>
     /\ x' = x

B == /\ pc = "B"
     /\ TRUE
     /\ pc' = "L"
     /\ UNCHANGED << x, stack, a >>

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ B \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
