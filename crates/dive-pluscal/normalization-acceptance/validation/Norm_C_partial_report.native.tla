---- MODULE Norm_C_partial_report ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { x:=1; E: skip; } } *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ x' = 1
         /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
