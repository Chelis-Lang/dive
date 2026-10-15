---- MODULE Norm_P_branch_inner_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0; begin if x=0 then x:=1; x:=2; else skip; end if; skip; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "a2ef9cd0" /\ chksum(tla) = "5d7cdb8d")
VARIABLES pc, x

vars == << pc, x >>

Init == (* Global variables *)
        /\ x = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF x=0
               THEN /\ x' = 1
                    /\ pc' = "Lbl_2"
               ELSE /\ TRUE
                    /\ pc' = "Lbl_3"
                    /\ x' = x

Lbl_2 == /\ pc = "Lbl_2"
         /\ x' = 2
         /\ pc' = "Lbl_3"

Lbl_3 == /\ pc = "Lbl_3"
         /\ TRUE
         /\ pc' = "Done"
         /\ x' = x

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Lbl_2 \/ Lbl_3
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
