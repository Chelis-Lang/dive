---- MODULE Norm_C_inner_repeat_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { while (x<2) { x:=x+1; x:=x; }; } } *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF x < 2
               THEN /\ x' = x + 1
                    /\ pc' = "Lbl_2"
               ELSE /\ pc' = "Done"
                    /\ x' = x

Lbl_2 == /\ pc = "Lbl_2"
         /\ x' = x
         /\ pc' = "Lbl_1"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Lbl_2 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
