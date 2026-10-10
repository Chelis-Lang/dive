---- MODULE Norm_C_simple_report ----
EXTENDS Naturals, TLC
(* --algorithm A { variables x=0; { x:=1; } } *)
\* BEGIN TRANSLATION (chksum(pcal) = "2d0b7386" /\ chksum(tla) = "849ddc79")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ x' = 1
         /\ pc' = "Done"

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
