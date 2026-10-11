---- MODULE C_while_test_valid ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A {variables x=0,y=0; { L:while((x<2)
  = TRUE){x:=x+1;}; E:skip; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "f97614cb" /\ chksum(tla) = "d4e42be1")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF ( x < 2 ) = TRUE
           THEN /\ x' = x + 1
                /\ pc' = "L"
           ELSE /\ pc' = "E"
                /\ x' = x
     /\ y' = y

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")
\* END TRANSLATION 

Good == pc # "E"
====
