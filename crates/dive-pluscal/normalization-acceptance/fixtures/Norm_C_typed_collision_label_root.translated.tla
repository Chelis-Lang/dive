---- MODULE Norm_C_typed_collision_label_root ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { Lbl_1: x:=1; x:=2; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "1e5d196d" /\ chksum(tla) = "428c0e51")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ x' = 1
         /\ pc' = "Step1"

Step1 == /\ pc = "Step1"
         /\ x' = 2
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Step1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
