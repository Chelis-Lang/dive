---- MODULE Norm_C_default_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x; { L: x:=1; E: skip; } } *)

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

====
