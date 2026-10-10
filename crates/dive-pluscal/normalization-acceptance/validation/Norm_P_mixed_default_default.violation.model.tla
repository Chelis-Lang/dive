---- MODULE Norm_P_mixed_default_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables x,y=0; begin L: x:=y; y:=1; E: skip; end algorithm; *)

CONSTANT defaultInitValue

VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = defaultInitValue
        /\ y = 0
        /\ pc = "L"

L == /\ pc = "L"
     /\ x' = y
     /\ y' = 1
     /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ UNCHANGED << x, y >>

Terminating == pc = "Done" /\ UNCHANGED vars

Next == L \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")


Good == pc # "Done"
====
