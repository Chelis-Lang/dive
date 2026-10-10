---- MODULE Norm_P_repeat_root ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0; begin x:=1; x:=2; end algorithm; *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "Step1"

Step1 == /\ pc = "Step1"
         /\ x' = 1
         /\ pc' = "Step2"

Step2 == /\ pc = "Step2"
         /\ x' = 2
         /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Step1 \/ Step2 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
