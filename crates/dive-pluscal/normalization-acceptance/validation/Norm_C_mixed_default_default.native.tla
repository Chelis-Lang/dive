---- MODULE Norm_C_mixed_default_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x,y=0; { L: x:=y; y:=1; E: skip; } } *)

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

====
