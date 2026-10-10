---- MODULE FairSequential_P_fair_plain_nof ----
EXTENDS Naturals,TLC
(* --fair algorithm A variable g=0,x=0; begin L: x:=1; E: g:=x; end algorithm; *)

VARIABLES pc, g, x

vars == << pc, g, x >>

Init == /\ g = 0
        /\ x = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = 1
     /\ pc' = "E"
     /\ g' = g

E == /\ pc = "E"
     /\ g' = x
     /\ pc' = "Done"
     /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == /\ Init /\ [][Next]_vars
        /\ WF_vars(Next)

Termination == <>(pc = "Done")

====
