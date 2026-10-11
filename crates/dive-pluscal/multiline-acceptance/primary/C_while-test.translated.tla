---- MODULE C_while_test ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:while(x<2
  = TRUE){x:=x+1;}; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "23ac9cb3" /\ chksum(tla) = "7e50c6d")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF                                              x<2
           = TRUE
           THEN /\ x' = x+1
                /\ pc' = "L"
           ELSE /\ pc' = "E"
                /\ x' = x
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
