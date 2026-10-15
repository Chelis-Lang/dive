---- MODULE Norm_C_variable_collision_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables Lbl_1=0; { Lbl_1:=1; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "bdfd037c" /\ chksum(tla) = "33d71696")
\* Label Lbl_1 at line 3 col 41 changed to Lbl_1_
VARIABLES pc, Lbl_1

vars == << pc, Lbl_1 >>

Init == (* Global variables *)
        /\ Lbl_1 = 0
        /\ pc = "Lbl_1_"

Lbl_1_ == /\ pc = "Lbl_1_"
          /\ Lbl_1' = 1
          /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1_
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
