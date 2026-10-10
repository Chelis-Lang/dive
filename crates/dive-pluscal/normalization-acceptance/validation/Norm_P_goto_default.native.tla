---- MODULE Norm_P_goto_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0; begin goto Done; x:=1; end algorithm; *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ pc' = "Done"
         /\ x' = x

Lbl_2 == /\ pc = "Lbl_2"
         /\ x' = 1
         /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Lbl_2 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
