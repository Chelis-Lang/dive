---- MODULE Norm_C_branch_label ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0,y=0; { if (x=0) { x:=1; } else { y:=2; }; x:=2; } } *)

VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF x = 0
               THEN /\ x' = 1
                    /\ y' = y
               ELSE /\ y' = 2
                    /\ x' = x
         /\ pc' = "Lbl_2"

Lbl_2 == /\ pc = "Lbl_2"
         /\ x' = 2
         /\ pc' = "Done"
         /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Lbl_2 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
