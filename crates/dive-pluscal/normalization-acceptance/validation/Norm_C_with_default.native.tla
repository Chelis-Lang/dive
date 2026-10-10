---- MODULE Norm_C_with_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { with (k=1) { x:=k; }; } } *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ LET k == 1 IN
               x' = k
         /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
