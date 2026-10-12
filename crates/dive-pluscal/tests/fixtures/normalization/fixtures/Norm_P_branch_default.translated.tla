---- MODULE Norm_P_branch_default ----
EXTENDS Naturals, TLC
(* --algorithm A variables x=0,y=0; begin if x=0 then x:=1; else y:=2; end if; x:=2; end algorithm; *)
\* BEGIN TRANSLATION (chksum(pcal) = "9412cf94" /\ chksum(tla) = "b0537767")
VARIABLES pc, x, y

vars == << pc, x, y >>

Init == (* Global variables *)
        /\ x = 0
        /\ y = 0
        /\ pc = "Lbl_1"

Lbl_1 == /\ pc = "Lbl_1"
         /\ IF x=0
               THEN /\ x' = 1
                    /\ y' = y
               ELSE /\ y' = 2
                    /\ x' = x
         /\ pc' = "Lbl_2"

Lbl_2 == /\ pc = "Lbl_2"
         /\ x' = 2
         /\ pc' = "Done"
         /\ y' = y

(* Allow infinite stuttering to prevent deadlock on termination. *)
Terminating == pc = "Done" /\ UNCHANGED vars

Next == Lbl_1 \/ Lbl_2
           \/ Terminating

Spec == Init /\ [][Next]_vars

Termination == <>(pc = "Done")

\* END TRANSLATION 
====
