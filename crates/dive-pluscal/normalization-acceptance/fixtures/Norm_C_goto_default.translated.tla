---- MODULE Norm_C_goto_default ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { goto Done; x:=1; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "f6976b6f" /\ chksum(tla) = "6e43fd13")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ pc' = "Done"
         /\ x' = x

Lbl_2 == /\ pc = "Lbl_2"
         /\ x' = 1
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Lbl_2
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
