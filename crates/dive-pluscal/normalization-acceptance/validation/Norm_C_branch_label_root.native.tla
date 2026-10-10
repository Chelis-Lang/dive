---- MODULE Norm_C_branch_label_root ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0,y=0; { if (x=0) { x:=1; } else { y:=2; }; x:=2; } } *)

VARIABLES pc, x, y

vars == << pc, x, y >>

Init == /\ x = 0
        /\ y = 0
        /\ pc = "Step1"

Step1 == /\ pc = "Step1"
         /\ IF x = 0
               THEN /\ x' = 1
                    /\ y' = y
               ELSE /\ y' = 2
                    /\ x' = x
         /\ pc' = "Step2"

Step2 == /\ pc = "Step2"
         /\ x' = 2
         /\ pc' = "Done"
         /\ y' = y

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Step1 \/ Step2 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
