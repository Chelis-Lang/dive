---- MODULE ElsifProcedure_C_with_call ----
EXTENDS Integers, Sequences, TLC
(* --algorithm A {variable x=0; procedure Put(z=0,a=0) {P:x:=a+z; R:return;}; {L:with (q=2) {if (x=0) {call Put(1,2); goto E;} else if (x=1) {skip;} else {call Put(1,2); goto E;};}; E:skip;}} *)
\* BEGIN TRANSLATION (chksum(pcal) = "df5b6cd4" /\ chksum(tla) = "cc48b1fa")
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
     /\ LET q == 2 IN
          IF x=0
             THEN /\ /\ a' = 2
                     /\ stack' = << [ procedure |->  "Put",
                                      pc        |->  "E",
                                      z         |->  z,
                                      a         |->  a ] >>
                                  \o stack
                     /\ z' = 1
                  /\ pc' = "P"
             ELSE /\ IF x=1
                        THEN /\ TRUE
                             /\ pc' = "E"
                             /\ UNCHANGED << stack, z, a >>
                        ELSE /\ /\ a' = 2
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
