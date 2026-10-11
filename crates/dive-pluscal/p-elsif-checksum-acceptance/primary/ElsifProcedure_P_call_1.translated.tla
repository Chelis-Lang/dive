---- MODULE ElsifProcedure_P_call_1 ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A variable x=0; procedure Put(z=0,a=0) begin P:x:=a+z; R:return; end procedure; begin L:if x=0 then skip; elsif x=1 then call Put(1,2); goto E; else skip; end if; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a3cca3d6" /\ chksum(tla) = "a4017260")
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
     /\ x' = a+z
     /\ pc' = "R"
     /\ UNCHANGED << stack, z, a >>

R == /\ pc = "R"
     /\ pc' = Head(stack).pc
     /\ z' = Head(stack).z
     /\ a' = Head(stack).a
     /\ stack' = Tail(stack)
     /\ x' = x

Put == P \/ R

L == /\ pc = "L"
     /\ IF x=0
           THEN /\ TRUE
                /\ pc' = "E"
                /\ UNCHANGED << stack, z, a >>
           ELSE /\ IF x=1
                      THEN /\ /\ a' = 2
                              /\ stack' = << [ procedure |->  "Put",
                                               pc        |->  "E",
                                               z         |->  z,
                                               a         |->  a ] >>
                                           \o stack
                              /\ z' = 1
                           /\ pc' = "P"
                      ELSE /\ TRUE
                           /\ pc' = "E"
                           /\ UNCHANGED << stack, z, a >>
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
