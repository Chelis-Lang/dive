---- MODULE Norm_P_simple_label_root ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0; begin x:=1; end algorithm; *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "Step1"

Step1 == /\ pc = "Step1"
         /\ x' = 1
         /\ pc' = "Done"

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Step1 \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
