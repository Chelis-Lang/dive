---- MODULE Norm_C_typed_collision_label_root ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { Lbl_1: x:=1; x:=2; } } *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ x' = 1
         /\ pc' = "Step1"

Step1 == /\ pc = "Step1"
         /\ x' = 2
         /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Step1 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
