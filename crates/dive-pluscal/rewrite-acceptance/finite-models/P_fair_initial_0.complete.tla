---- MODULE Rewrite_P ----
EXTENDS Integers
(* --fair algorithm A variable x=0; begin L:x:=1; E:skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) \in STRING /\ chksum(tla) = "42ff5319")
VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")
\* END TRANSLATION 
\* preserved footer
====
