---- MODULE Norm_P_default_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables x; begin L: x:=1; E: skip; end algorithm; *)

CONSTANT defaultInitValue

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = defaultInitValue
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

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")


Good == pc # "Done"
====
