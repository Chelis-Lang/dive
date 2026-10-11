---- MODULE ElsifProcedure_P_either_return ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0) begin P:either if x=0 then return; elsif x=1 then skip; else return; end if; or skip; end either; R:return; end procedure; begin L:call Put(1,2); goto E; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "3ac94300" /\ chksum(tla) = "701c3f95")
VARIABLES pc, x, stack, z, a

vars == << pc, x, stack, z, a >>

Init == (* Global variables *)
        /\ x = 0
        (* Procedure Put *)
        /\ z = 0
        /\ a = 0
        /\ stack = << >>
        /\ pc = "L"

P == /\ pc = "P"
     /\ \/ /\ IF x=0
                 THEN /\ pc' = Head(stack).pc
                      /\ z' = Head(stack).z
                      /\ a' = Head(stack).a
                      /\ stack' = Tail(stack)
                 ELSE /\ IF x=1
                            THEN /\ TRUE
                                 /\ pc' = "R"
                                 /\ UNCHANGED << stack, z, a >>
                            ELSE /\ pc' = Head(stack).pc
                                 /\ z' = Head(stack).z
                                 /\ a' = Head(stack).a
                                 /\ stack' = Tail(stack)
        \/ /\ TRUE
           /\ pc' = "R"
           /\ UNCHANGED <<stack, z, a>>
     /\ x' = x

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ z' = Head(stack).z
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

L == /\ pc = "L"
     /\ /\ a' = 2
        /\ stack' = << [ procedure |->  "Put",
                         pc        |->  "E",
                         z         |->  z,
                         a         |->  a ] >>
                     \o stack
        /\ z' = 1
     /\ pc' = "P"
     /\ x' = x

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, stack, z, a >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Put \/ L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
