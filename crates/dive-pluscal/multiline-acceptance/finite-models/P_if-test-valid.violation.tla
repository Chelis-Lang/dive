---- MODULE P_if_test_valid ----
EXTENDS Integers, TLC, Sequences
(* --algorithm A variables x=0,y=0; begin L:if (y=0)
  = TRUE then x:=1; else x:=2; end if; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "baaf01a7" /\ chksum(tla) = "2b9adbae")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ IF ( y = 0 ) = TRUE
           THEN /\ x' = 1
           ELSE /\ x' = 2
     /\ pc' = "E"
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
