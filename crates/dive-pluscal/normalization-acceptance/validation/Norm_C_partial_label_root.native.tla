---- MODULE Norm_C_partial_label_root ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { x:=1; E: skip; } } *)

VARIABLES pc, x

vars == << pc, x >>

Init == /\ x = 0
        /\ pc = "Step1"

Step1 == /\ pc = "Step1"
         /\ x' = 1
         /\ pc' = "E"

E == /\ pc = "E"
     /\ TRUE
     /\ pc' = "Done"
     /\ x' = x

Terminating == pc = "Done" /\ UNCHANGED vars

Next == Step1 \/ E \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

====
